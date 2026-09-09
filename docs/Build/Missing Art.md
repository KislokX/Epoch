# Art the build is waiting on

> **Why this file exists.** `CLAUDE.md`: *assets are authored, never generated* — when
> implementation would benefit from new artwork, implementation **pauses and names what is
> missing** rather than inventing it or building a system to hold it.
>
> Everything here is a place where the code is finished and the drawing is not. Nothing here is
> blocking a working product: each one has an honest fallback that ships today.

---

## `ui.frame.window` — **supplied 2026-08-09**

Authored by the project owner and shipped in the default pack as
`packs/default/assets/ui/window.png` (300×200). Every window in Epoch is drawn from it.

What it settled, by being real rather than imagined:

- **Corners are four numbers, not one.** Measured from the artwork: top 11, right 5, bottom 12,
  left 6. A bevel lit from above is not symmetric, and one number would have stretched its
  highlight or eaten into its face.
- **`stretch`, not `repeat`.** The face is a soft vertical gradient; tiling it bands. A pattern
  with no gradient across it would say `repeat`, so the author says which.
- **A scale field.** Whole numbers only — Epoch is pixel art and a fractional scale destroys it.
  `0` is refused rather than clamped, so a typo is visible.
- **A minimum size, derived.** Two opposite corners plus a middle at least as long as the larger
  of them. Below that the corners meet and the frame stops being a frame, so the window does not
  go there — refusing an impossible size beats handling it.

**Licence — answered 2026-08-10, and the answer is a blocker for distribution.** The file is the
project owner's own artwork and **is not cleared for distribution**. It is correct to develop
against and it may not ship.

CONTENT_PHILOSOPHY's hard rule governs what Epoch *distributes*, not what a user keeps in their
own vault — but `packs/default/` is not a user's vault. It is the shipped pack, and its manifest
declares `CC0-1.0` over everything in it. That declaration is currently wider than the truth.

Two things must happen before a release, and neither is code:

1. `packs/default/assets/ui/window.png` is replaced by an original or CC0 window, **or** its
   author releases this one under the pack's declared licence.
2. The same question is answered for `packs/default/assets/preview.png`, which arrived the same
   way.

Until then: no distribution build of the default pack. Nothing in the Engine depends on the
answer — the resolver treats any window the same — so this is a release gate, not a blocked
implementation.

---

## Walking frames — the crew, in motion

**Status:** blocked. Named in `ROADMAP.md` as Phase 3.3 (*actions, not animations*).

**What is needed:** sprite sheets for a character in motion. Four directions, or two if the World
is drawn side-on.

**Why the contract is not built yet:** the Engine would name `walk.east`, `idle`, `work` and a
pack would map those to frames — but building that mapping with nothing to map is the same
imagined-use abstraction the `ui.*` namespace is deliberately avoiding. The first real sheet says
what the contract needs.

**What ships until then:** a walking character reuses their standing sprite with a gait — a small
vertical bob and a shadow that lifts with it, quicker when they are carrying work. It reads as
movement and it is honest about being one drawing.

---

## The Official Epoch Universe

**Status:** a deliberately deferred milestone, with its own document:
[docs/Milestones/Official Epoch Universe.md](../Milestones/Official%20Epoch%20Universe.md).

Cast, lore, cities, visual identity, music direction, logos. Not a gap in the build — the engine
knows only archetypes and place concepts (ADR-0017), so the universe is content that arrives
later without the code changing. Listed here so that "Epoch has no art of its own yet" is a
decision somebody can find, rather than something that looks like an oversight.


---

## The application icon for EpochServices — decided

**Status:** not missing. **EpochServices wears Epoch's own mark, on purpose.**

This was written up as a gap and it was the wrong reading. The two programs are one product on
two machines: the companion exists only because a Host asked it to, and it lends its hardware to
that Host and to nothing else. A separate mark would have said they were separate things.

**How it is built:** the full set is rendered from the authored `icon-1024.svg` at the repository
root into `EpochServices/icons/`, and `tauri.conf.json` lists the five files a bundle needs.
`icon.icns` is among them, which is what lets macOS produce a `.dmg` from the same configuration
that produces a `.msi` on Windows.

**Measured, and worth keeping written down:** a bundle with no icon does not build at all —
`failed to bundle project: Couldn't find a .ico icon`. The icon is not decoration here; it is a
build input.

**To change it later:** `cargo tauri icon path/to/new-1024.png` inside `EpochServices/`.

