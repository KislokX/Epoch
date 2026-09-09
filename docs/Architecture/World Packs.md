# World Packs (narrative + asset resolution)

> Status: Designed · Owner ADRs: [[../ADR/0016-asset-resolution]], [[../ADR/0017-character-archetypes-world-packs]]
> Constitution: [[../../CONTENT_PHILOSOPHY]] · Horizon: vocabulary + manifest + resolver + fallback + placeholder default pack IMPLEMENT NOW; official universe VISION ([[../Milestones/Official Epoch Universe]])

## Purpose
Keep the engine **universal and franchise-free** while the entire identity of the World — characters, names, places, vocabulary, art, music — becomes replaceable data.

**The engine stays universal. The World Pack gives it identity.**

## Shape
The engine references **canonical concepts**. The active World Pack projects them into an actual world.

```
Engine:  render(character.researcher, walk.east)
         enter(place.knowledge_center)
         play(mood.research)
              |
              v
       World Pack Resolver (stateless)
              |
   active pack -> base pack -> default pack -> visible placeholder
```

## Canonical concepts (Domain Kernel, versioned, additive-only)

**Character archetypes** — the engine knows *only* these, never names:
```
character.researcher    character.coordinator
character.guardian      character.historian
```

**Place concepts:**
```
knowledge_center   research_lab   automation_hub
command_center     guild
```

**Assets** — animation states (`idle`, `walk.<dir>`, `work`, `talk`, `travel`), tilesets, icons, UI, fonts, particles, weather, lighting, cursor, transitions, SFX, and **music by mood** (`mood.research`, `mood.peaceful_town`, `mood.battle`, `mood.celebration`, `mood.night`, `mood.rain`, `mood.danger`).

The engine requests a **mood**, never a track. A **concept**, never a filename. An **archetype**, never a character.

## What a World Pack supplies
A complete **narrative projection**, not a skin:
- character names, portraits, sprites, personality presentation
- location names and appearance
- world vocabulary (Era, Quest, Chronicle, Party, History — pack-supplied labels)
- all assets: sprites, music, SFX, fonts, UI colors, windows, icons, particles, animations, weather, lighting, cursor, transitions

A Chrono-inspired pack, a Sci-Fi pack and a Fantasy pack map the same four archetypes to entirely different casts. The engine cannot tell the difference.

## Manifest
Identity + version · **concept coverage** (a Profile, queryable like any component per [[../ADR/0005-capability-first-architecture]]) · **mandatory license metadata**.

## Resolver
Stateless, like the Character Runtime: `resolve(concept, variant, pack_chain) -> Handle`. Resolve once, cache per pack activation.

## Fallback chain (required)
active pack -> base/parent pack -> default pack -> **visible placeholder**.
Never crash. Never render a blank. Unresolved concepts reported via [[Observability]].

## NOT responsible for
- **Behavior of any kind.** Packs are content, never Definitions ([[Definition Runtime]]). A pack changes what things are *called and look like*, never what a Character *does*.
- Engine logic or the rules of motion ([[../../LIVING_WORLD_DESIGN_GUIDE]] governs *how* the World behaves, for every pack).
- Hosting or vetting third-party packs (Phase 1).

## Default pack (today)
Ships **archetype-descriptive placeholders** — "The Researcher", "The Coordinator", "The Guardian", "The Historian" — and neutral place labels. The official Epoch universe is a dedicated milestone, deliberately deferred.

## Hard rule
Official distribution ships only original / commissioned / open-source / public-domain-CC0 / explicitly redistributable content. Never copyrighted game assets, casts, or locations. Inspiration is welcome; identity must be original.

## Scope
Active World Pack = **Application-scope** setting ([[Scope Model]]), optional Workspace override.

## Risks
- Vocabulary churn breaking packs -> versioned, additive-only, deprecate never remove.
- Incomplete packs -> fallback chain + coverage reporting + obvious placeholder.
- Infringing community packs -> mandatory license metadata; Epoch does not host/endorse unvetted packs.
- Render-time indirection -> resolve once, cache per activation.

## Open questions
- Pack format/packaging + install location.
- Pack inheritance / partial packs over a base (VISION).
- Whether world vocabulary lives in the manifest or a separate localisation layer.
- How a pack declares coverage when it adds original NPCs beyond the four archetypes.
