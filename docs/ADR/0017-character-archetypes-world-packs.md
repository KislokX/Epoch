# ADR-0017: Character Archetypes & World Packs

- Status: Accepted, amended by [[0023-characters-belong-to-the-user]]
- Date: 2026-07-25
- Depends on: [[0011-definition-runtime]], [[0013-scope-model]], [[0016-asset-resolution]]
- Amends: [[0016-asset-resolution]] (Theme Pack -> **World Pack**; scope extended from assets to full narrative projection)
- Amended by: [[0023-characters-belong-to-the-user]] — **a World Pack no longer supplies the cast.**
  Character names, portraits and sprites moved to the user's vault, and `CharacterId` was
  introduced so archetype could go back to classifying rather than identifying. What survives
  intact is this ADR's load-bearing claim: the engine knows only archetypes and place concepts,
  and never a name. It simply receives the name from the vault instead of from a pack. The part
  that is gone is a pack's ability to re-cast the crew — see ADR-0023 §2 for why that was worth
  losing. **Sections below describing `[characters.*]` in the manifest are historical.**
- Related architecture: [[World Packs]], [[Domain Kernel]], [[../../CONTENT_PHILOSOPHY]], [[../../CHARACTER_BIBLE]]
- Horizon: archetype + place concept vocabulary, World Pack narrative manifest, placeholder default pack **IMPLEMENT NOW**; official Epoch universe **VISION (dedicated milestone)**


> **Amended 2026-08-05 by [ADR-0028](0028-the-map-belongs-to-the-user.md).** A World Pack no
> longer supplies the map, and the five place concepts are neither frozen nor exhaustive. The
> Engine still never learns a name — but the names are the user's, exactly as ADR-0023 made the
> cast theirs.

## Context
ADR-0016 made assets replaceable but the engine still referenced characters and places by name (`lucca`, "End of Time", "Chronopolis"). Those names come from *Chrono Trigger* / *Chrono Cross*. Beyond the legal exposure of shipping a product whose cast is identifiably another company's, it contradicted our own rule that the engine must never reference a franchise. Inspiration was leaking into the engine.

## Decision
**The engine knows only canonical archetypes and place concepts. It never knows a character name, a location name, or a franchise.**

**Character Archetypes (Domain Kernel):**
```
character.researcher    character.coordinator
character.guardian      character.historian
```

**Place concepts (Domain Kernel):**
```
knowledge_center   research_lab   automation_hub
command_center     guild
```

**World Packs** (renamed from Theme Pack) are **complete narrative projections of the engine**, not visual themes. A World Pack projects archetypes and place concepts into an actual world, supplying:
- character **names**, portraits, sprites, personality presentation
- location names and appearance
- world vocabulary (Era, Quest, Chronicle, Party, History are *pack-supplied* labels)
- all assets from ADR-0016 (sprites, music by mood, SFX, fonts, UI, particles, weather, lighting, cursor, transitions)

A Chrono-inspired pack could map Researcher -> its own character; a Sci-Fi or Fantasy pack maps the same archetypes to entirely different casts. **The engine stays universal; the World Pack gives it identity.**

**The default World Pack ships archetype-descriptive placeholders** ("The Researcher", "The Coordinator", "The Guardian", "The Historian") and neutral place labels. We deliberately do **not** invent a cast now.

**The official Epoch universe is a dedicated future milestone** (see [[../Milestones/Official Epoch Universe]]) covering characters, names, lore, cities, buildings, history, timeline, visual identity, music direction, NPCs, factions, symbols, logos, and the official default World Pack. It is the birth of Epoch's own universe — not a renaming pass.

## Why this is best
- Internal First made complete: the engine's vocabulary is now entirely franchise-free and universal.
- Removes structural IP exposure at its root rather than patching names.
- World Packs become far more valuable — a pack can retell the entire product in a different fiction without touching the engine.
- Protects the creative work: the official cast gets a deliberate milestone instead of being rushed into placeholders.

## Alternatives considered
- **Rename the cast now to original names** — solves legal exposure but leaves names welded into the engine and forces rushed creative decisions. Rejected.
- **Character-specific ids (`character.lucca`)** — still franchise-bound; violates ADR-0016's own rule. Rejected.
- **Keep the Chrono Trigger names** — structural legal exposure for a 5-year product intended to become a platform. Rejected.

## Consequences
- `CharacterDefinition.Identity` references an **archetype**, not a name. Display name comes from the active World Pack.
- The Character Bible is restructured around archetypes; behavioural design (decision style, risk posture, communication) survives because it projects into Prompt / Capabilities / Trust Policies. Names, portraits and personality *flavour* move to the World Pack.
- World vocabulary (Era/Quest/Chronicle/Party/History) is pack-supplied presentation, not fixed product terminology.
- Existing docs referencing the old cast and locations are updated in this pass.

## Assumptions
- Four archetypes cover Phase-1 needs; more can be added additively.
- Behavioural identity can be expressed per-archetype without a specific character attached.

## Risks
- **Placeholder names feeling lifeless** during development. *Mitigation:* accepted deliberately; the milestone exists precisely to fix it well.
- **Archetype vocabulary churn.** *Mitigation:* versioned, additive-only, like the Capability vocabulary.
- **Packs carrying infringing casts.** *Mitigation:* mandatory license metadata (ADR-0016); Epoch neither hosts nor endorses unvetted packs.

## Open questions
- Whether world vocabulary strings live in the World Pack manifest or a separate localisation layer.
- How a pack declares archetype coverage when it adds original NPCs beyond the four.
