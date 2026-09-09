# Milestone: Create the Official Epoch Universe

> Status: **Scoped 2026-08-21 by the owner** — most of it turned out to be things that already
> exist under other names · Horizon: the World Edit work **IMPLEMENT NOW**, the artwork itself
> **VISION**
> Owner ADR: [[../ADR/0017-character-archetypes-world-packs]], amended by
> [[../ADR/0023-characters-belong-to-the-user]]

## What this was, and why it was rewritten

It was a list of twelve creative deliverables — cast, names, lore, cities, history, timeline,
visual identity, music direction, NPCs, factions, symbols, logos — deferred because *"we do not
yet know who these Characters truly are"*.

That reasoning was right and is now mostly **spent**, for two separate reasons.

**ADR-0023 took the cast out of the World Pack.** Characters belong to the user and travel into
every World. So the official universe cannot be *"the four people Epoch ships"* — there is no
such thing any more, and inventing them would re-open a decision already made.

**And the owner mapped the rest onto things that exist** (2026-08-21). Asked what each item
actually *is* in the built product:

| Was listed as | Is actually | State |
|---|---|---|
| Character names | *(gone — ADR-0023)* | The user's crew |
| World name | The pack's `name` | **Exists** |
| Lore, history, timeline | Missions | **Exists** |
| Cities | Buildings — Places | **Exists** |
| Factions | — | **Discarded by the owner** |
| Symbols | — | **Discarded by the owner** |
| Logo | The mark EpochServices already ships | **Exists** |
| Sprites, tiles, portraits, backdrops | Skinning the World's interface — what `window.png` already does | **Half exists: no way to set it** |
| Music, SFX | Sound for a World | **Does not exist** |

What is left is therefore **not a creative milestone at all**. It is two pieces of engineering,
and then artwork made outside the Engine, exactly as *assets are authored, never generated*
requires.

## The work

### 1. World Edit can set the World's skin

The pipeline is built and used: a pack declares `[[ui]]` entries mapping an Asset Concept
(`ui.frame.window`, `ui.frame.tooltip`) to an image with corner insets, `WorldPack::discover`
resolves them along the pack chain, and the interface draws the highest one that supplies each
concept. An undeclared concept is not a gap — Epoch draws its own.

**What is missing is the door.** Today that is hand-edited TOML plus a file dropped in a folder,
which is exactly the thing this product exists not to require.

So: World Edit gains a skin section. One row per concept the engine knows, each showing what is
supplying it now, with import and clear. Imports travel the pipeline that already exists
(ADR-0024): bytes from the webview's own file input, base64 across IPC, and only the Engine
writes — named from the bytes, never from the uploaded filename.

**The list of concepts is the engine's, not the editor's.** A frontend holding its own list would
drift the first time a concept is added, which is the mistake ADR-0003 was written about.

### 2. World Edit can give a World sound

This one is genuinely new, and the honest description is that Epoch has no audio pipeline at all:

- **SFX are synthesised.** `sfx.ts` is a small Web Audio voice bank with no files, deliberately —
  it ships with the desktop application and makes no request on first paint. A pack supplying a
  sound must therefore *override a voice*, not fill an empty slot.
- **Music does not exist.** Not silent — absent. `CONTENT_PHILOSOPHY` already says music is
  requested as a **mood** and never as a track, so the concept namespace is `music.<mood>` and the
  engine never learns a filename.
- **`asset.rs` delivers images.** Audio needs its formats added there, and the rule that the
  accepted list matches what the asset layer can actually deliver is held by a test — so this is
  one place to extend, not two lists to keep agreeing.

The fallback chain is the same as every other concept: active pack → base → default → the
synthesised voice. A World with no sound is complete, not unfinished.

### 3. Then, and only then, the artwork

With both doors open, creating the official universe requires **replacing only a World Pack** —
which was the precondition ADR-0016 set and the whole point of the concept indirection.

That work is drawing, composing and writing. It is not engineering, it does not happen in this
repository, and `CONTENT_PHILOSOPHY`'s hard rule governs it: original, commissioned, open-source
or public-domain only. Never generated — and ADR-0030 keeps that true from the other direction by
refusing to let a generated image become World Pack artwork.

## What the official universe now means

Two things, and they ship separately:

**A World.** Land, roads, buildings, the interface's own skin, its sound, its name, and the
Missions that carry its lore. This is the part a pack supplies, and it is the part that makes
Epoch look like Epoch.

**A crew somebody can start from.** A Character Pack (ADR-0026) the user imports, keeps, renames
or deletes. Not *the protagonists of Epoch* — a starting point, because the crew is theirs.

## Until then

The default World Pack ships archetype-descriptive placeholders — "The Researcher", "The
Coordinator", "The Guardian", "The Historian" — and neutral place labels. Zero creative
investment, zero borrowed identity, and the gap stays visible.

## Preconditions

- Engine archetypes + place concepts stable ([[../ADR/0017-character-archetypes-world-packs]]) —
  **met**.
- World Pack contract implemented ([[../ADR/0016-asset-resolution]]) — **met**.
- Skin and sound editable from World Edit — **the work above**.

---

> Architecture gives Epoch a brain.
> The World gives it a soul.
> Its own universe will give it an identity — and by then, only a pack has to change.
