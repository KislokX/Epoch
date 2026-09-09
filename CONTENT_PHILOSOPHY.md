# Epoch Content Philosophy

> A permanent design constitution — the fifth pillar.
> Companion to [EXPERIENCE_CONSTITUTION.md](EXPERIENCE_CONSTITUTION.md) (soul), [CHARACTER_BIBLE.md](CHARACTER_BIBLE.md) (heart), [LIVING_WORLD_DESIGN_GUIDE.md](LIVING_WORLD_DESIGN_GUIDE.md) (body language).
> Engine contracts: [ADR-0016](docs/ADR/0016-asset-resolution.md), [ADR-0017](docs/ADR/0017-character-archetypes-world-packs.md).

---

## The principle

**The Living World belongs to the user — not to a particular art style, and not to a particular fiction.**

The default world is only the *first interpretation*. The engine creates the experience; the **World Pack** gives it identity.

The Living World must never depend on copyrighted assets, casts or locations. Epoch is designed from day one for **replaceable content**.

---

## Engine First


**Character archetypes:**
```
character.researcher    character.coordinator
character.guardian      character.historian
```

**Place concepts:**
```
knowledge_center   research_lab   automation_hub
command_center     guild
```

**Assets, environment, audio:**
```
walk.east      idle       work        talk
tile.forest    time.night time.sunset rain
mood.research  mood.peaceful_town     mood.battle
```

Example request:
```
Character:  character.researcher
Animation:  walk.east
Music:      mood.research
```

The **active World Pack decides** which name, portrait, sprite and track to use. The engine neither knows nor cares whether it came from the default pack, a community pack, a premium pack, or one the user made this morning.

This keeps the engine completely independent from art style **and from fiction**.

---

## World Packs

A World Pack is a **complete narrative projection of the engine** — not a skin. It supplies:

**Identity:** location names · world vocabulary (Era, Quest, Chronicle, Party, History)

> **Amended by ADR-0023 (2026-07-29).** Character names, portraits and sprites are **not** pack
> content. The crew belongs to the user: their name, face, personality, role and routine live in
> the vault and travel with them into every World. A pack dresses the *world*, never the people
> in it. This was decided when the model met the product: a World is a Workspace, agents are
> assigned to Workspaces, and an agent whose name changes depending on which project it is in
> is not portable in any sense a user would recognise.

**Assets:** character sprites · NPC sprites · buildings · tilesets · animations · icons · UI decorations · fonts · sound effects · music · ambient sounds · weather effects · particles · lighting · cursor · transitions · UI colors · windows

Everything together creates a coherent world.

### Music by mood
The engine requests **moods**, never tracks: `mood.peaceful_town`, `mood.research`, `mood.battle`, `mood.celebration`, `mood.night`, `mood.rain`, `mood.danger`. The active pack decides what plays.

### Sound
Likewise for UI sounds, footsteps, doors, magic, notifications and environment. Everything is replaceable.

---

## What a World Pack may not do

A pack may freely reinvent place names, voice and setting. It **may not change behaviour**. An archetype's values, decision style, risk posture and relationships are engine-side and constant. A pack that makes the Guardian reckless is not a theme — it is a different product.

Since ADR-0023 it also **may not supply the cast**. Character names, portraits and sprites are the user's, not the pack's. A per-World presentation *override* may be reintroduced if a themed World ever needs one — but it would be an override over an identity that already exists, never the identity itself.

Packs are **content, never Definitions**.

### The default pack today
Ships **archetype-descriptive placeholders**: "The Researcher", "The Coordinator", "The Guardian", "The Historian", and neutral place labels. We deliberately do not invent a cast as filler — the official universe is its own creative milestone: [Create the Official Epoch Universe](docs/Milestones/Official%20Epoch%20Universe.md).

## Community

Eventually the community should be able to create complete Worlds — the way Skyrim has mods, Minecraft has resource packs, Stardew Valley has its modding scene.

**Epoch becomes a platform, not only an application.**

---

## What this obligates us to

1. Every visual, audio, typographic **and naming** element is data-driven — resolved through a pack, never hardcoded.
2. The engine's **concept vocabulary** is canonical and versioned (like the Capability vocabulary). Concepts are the contract; identity is the projection.
3. A missing concept **degrades gracefully** — active pack → base pack → default pack → visible placeholder. Never a crash, never a blank.
4. Packs declare **coverage** and **license** in a manifest.

---

**The engine stays universal. The World Pack gives it identity.**