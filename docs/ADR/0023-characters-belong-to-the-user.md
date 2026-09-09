# ADR-0023: Characters belong to the user

- Status: Accepted
- Date: 2026-07-29
- Depends on: [[0011-definition-runtime]], [[0016-asset-resolution]], [[0018-world-simulation]]
- Amends: [[0017-character-archetypes-world-packs]] (a World Pack no longer supplies the cast), [[0016-asset-resolution]] (a character's artwork is not pack content)
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../CONTENT_PHILOSOPHY]], [[../../CHARACTER_BIBLE]]
- Horizon: character identity, the roster, and Launcher editing **IMPLEMENT NOW**; per-World presentation overrides, character import/export, animated appearance **DESIGN NOW**; a character marketplace **VISION**

## Context — the evidence

ADR-0017 gave the World Pack the cast: the engine knew four archetypes, and a pack supplied each one's name and face. A Chrono-inspired pack, a sci-fi pack and a fantasy pack would map the same four archetypes to entirely different casts, and the engine could not tell the difference.

Building the Launcher broke that model in three places, in ascending order of severity.

**1. The Launcher reported a population nobody had created.** "The Archipelago" claimed four characters when one existed. The number counted *names a World supplied for archetypes* — the only character-shaped thing a pack had. There was no honest number to show, because the fact "who lives here" existed nowhere.

**2. Archetype was doing the work of an identity.** `PresenceState`, `WorldPackChain::character()`, `Simulation::populate` and the Definition registry were all keyed by `CharacterArchetype`. That is workable only while at most one character exists per archetype. The moment a user wants two researchers, the second silently replaces the first — and which one survives depends on directory order.

**3. The model contradicted what a World is.** `PRODUCT_ARCHITECTURE.md` defines a World as a Workspace plus its World Pack, and a Workspace is a project. Agents are assigned to projects. A user moving an agent from one project to another is ordinary; an agent whose *name changes* depending on which project it is in is not. Under ADR-0017 the same crew member was "The Researcher" in one World and "The Cartographer" in another, and neither name was theirs.

This is exactly the distinction ADR-0021 drew for Places, arrived at from the opposite direction:

```
PlaceConcept        what capability this projects        classification
PlaceId             which Place this is                  identity
```

Places got both. Characters had only the first.

## Decision

### 1. Identity is separate from classification

```
CharacterArchetype   what kind of worker they are   classification
CharacterId          who they are                   identity
```

`CharacterId` is a validated newtype in the Kernel, constrained to a file-safe alphabet because a character owns files in the vault. Several characters may share an archetype and remain several people. Nothing keys on a display name, ever — renaming somebody must not make them a different person.

### 2. The cast belongs to the user, not to a World

A character's **name, face, personality, role and routine** live in their Definition in the vault. They travel with the character into every World.

A World Pack supplies **places, land, roads, vocabulary and artwork**. It declares no characters at all. The `[characters.*]` section is removed from the manifest format.

This amends ADR-0017's central claim. What survives is the part that was actually load-bearing: **the engine knows only archetypes**. It still never learns a name — it just receives one from the vault instead of from a pack.

What is given up is the ability for a pack to re-cast the crew. That was a real property and it is genuinely lost. It was worth losing: it served theming, and it cost the user ownership of their own crew. A per-World *presentation override* can be reintroduced if a themed World ever needs one, and it would be an override over an identity that exists — not the identity itself.

### 3. The roster lives with the character

Each character declares which Worlds they live in:

```toml
worlds = ["default", "archipelago"]
```

Not the other way round. Two reasons, and the first is decisive:

- **A World Pack is shipped content.** It cannot know that you invented somebody. Putting the roster in the pack would mean every user edit modifies a file they did not author and which an update would overwrite.
- **It matches the user's mental model.** "Move Mage to the other World" is a thing you do *to Mage*.

Moving a character between Worlds is therefore an edit to one file, and it works identically whether or not that World is open.

### 4. A character's artwork is theirs

Appearance is authored in the Definition — a sprite path, a scale and an anchor — and resolved against the vault's characters folder rather than a World directory.

It goes through the **identical** pipeline a Place's artwork travels: same `Mark`, same asset resolution, same path confinement, same `data:` URI treatment (ADR-0020), same honest degradation. A character's appearance is not a second renderer.

One deliberate difference, and only one. A Place is several marks: one whose asset cannot be read still leaves a building standing, drawn by the others. A character *is* that one mark — keeping it would produce a sprite with nothing to draw, an invisible person, which is worse than an absent face because nothing downstream would know anything was missing. So an unreadable character sprite resolves to `None`, is reported, and the World draws the visible stand-in it already knows how to draw.

### 5. Coverage is places alone

`WorldPack::coverage()` counted place concepts *and* character archetypes. With characters gone from packs, it counts places. Full coverage now genuinely means "this World described everywhere the engine knows", rather than being permanently capped by a thing packs no longer supply.

### 6. Editing happens in the Launcher, over the file

The Launcher is where the crew is managed — the Launcher prepares, the World immerses (`PRODUCT_ARCHITECTURE.md`). It is an **editor over the vault, not a parallel store**: it writes the same file it reads and re-reads afterwards, so the file stays the source of truth and the existing hot reload carries the change into an open World with no extra mechanism.

Validation lives in the Engine, never in the surface. A surface that validates separately will eventually disagree with the thing that has to live with the file.

## Consequences

**Good**

- The Launcher can state a true population, because "who lives here" is now a fact that exists.
- Two characters of one archetype are two people. This was previously silent data loss.
- A crew member is recognisably themselves in every World, which is what makes them *yours*.
- Deleted rather than added: `CharacterDeclaration`, `CharacterContribution`, `WorldPackChain::character`, `character_mark`, and `Resolved` — whose last caller was the character name.
- `PlaceConcept::from_id` and `CharacterArchetype::from_id` now exist, so a choice arriving from a surface is refused by the Engine rather than trusted.

**Costs**

- A World Pack can no longer present a different cast. Named and accepted above.
- Every subsystem touching characters changed: Kernel, registry, simulation, packs, projection, wire contract, shell and UI. This is the largest refactor since Places, and it was cheaper now than it will ever be again.
- Character artwork no longer ships inside a World, so a World is no longer a single self-contained folder containing everything you see in it. Export will have to decide whether it carries the crew.

**Discovered while building this**

The registry inferred "how many files were on disk last time" from `characters.len() + problems.len()`. Once one character could contribute a problem *and* load successfully, that inference broke, and `changed_on_disk()` would have returned true forever — an infinite reload loop, at 2 Hz, in the shipped tick. Counting files directly fixed it. The lesson is small and repeatable: **a derived count is not a substitute for the thing it approximates**, and this one had been correct only by coincidence.

## Alternatives considered

**Keep name and face in the pack, move only personality.** The first shape proposed. Rejected by the mage/paladin example: a crew member whose *name* changes per World is not portable in any sense the user would recognise.

**Put the roster in the pack.** Rejected — see §3. A shipped pack cannot know the user's characters.

**Key characters by archetype and allow duplicates via a suffix.** Rejected: that is an identity with extra steps, and it would have left every existing lookup ambiguous rather than fixing it.

**Keep `Resolved` for symmetry.** Rejected. `Place::is_placeholder` already answers "did anyone actually supply this?" for a whole composed Place. Two ways to ask one question is how vocabularies rot.

## Amendment (2026-08-19): a drawing per action

A character had two drawings — the sprite that walks the World and the icon that identifies
them — addressed by a boolean. There are more than two now, so the boolean became a **slot**:
`sprite`, `icon`, or one of the four canonical actions (`idle`, `walk`, `think`, `work`).

**Every slot is independent, and each has its own file stem.** Two drawings can never collide,
and clearing one can never delete another's file. That was already true of the sprite and the
icon; it is the rule rather than a coincidence now.

**Actions are additive, never required.** An action nobody drew falls back to the still sprite,
and a character with no sprite either falls back to the visible stand-in. That is ADR-0016's
chain, untouched: a character with one drawing is *complete*, and animation is something a World
gains rather than something it needs.

### The same pipeline, again, and this time it costs nothing to say so

An action's sheet becomes a `MarkDeclaration` with the `animated_sprite` renderer and its
authored cut, and `resolve_mark` does the rest — the same path confinement, the same size limit,
the same `data:` URI, the same judgement that a mark with nothing to draw is an invisible person
and is dropped. A character's artwork has never been a second renderer, and per-action artwork
does not make it one.

The **cut itself lives in the Kernel** (`Sheet`), shared with the pack-authored version from
ADR-0021. Both authors of artwork declare exactly the same thing; two shapes would have drifted
the first time somebody compared them.

### Refused at the door, in both directions

A sheet with no cut is refused rather than written, so a strip can never reach the vault with no
way to read it. A cut sent for the sprite or the icon is refused too — those are pictures. A
surface that sends the wrong pair hears about it instead of watching one half disappear.

### A word this build does not know costs one entry

Actions are keyed by **string** on disk, not by the enum. A file written by a later Epoch — one
that knows `talk`, or `sleep` when its cause exists — must still open in this one: the unknown
entry survives the round trip in the file, resolves to nothing, and is reported. The same rule
as an unknown mark role, applied to somebody's artwork.

### The editor watches it play (2026-08-19)

A cut is four numbers, and **four numbers cannot be checked by reading them**. A sheet declared
`4×4` that is really `4×3` looks perfectly reasonable in a form and produces somebody walking
with another character's legs. So each animation slot previews the sheet *playing*, in reading
order across every real cell — which for a directional sheet walks through all four directions
in turn, because the question being answered is *did I cut this right* and one row would only
answer it for one row.

**Nothing in the form is guessed.** The grid and the duration start empty and IMPORT stays
disabled until they are stated, which is the Engine's rule surfaced rather than a second one: a
default frame rate would be a constant nobody measured. A direction word this build does not
know is refused here too, so the answer arrives while the user is still looking at the field
instead of as an error after an upload.

**Re-cutting is its own act.** `set_cut` changes how a sheet is read without touching the sheet:
nothing is imported, nothing is named, no file is written or removed. Fixing a typo in `rows`
should be two numbers rather than the same PNG uploaded again — and that loop, *state · import ·
watch · fix*, is what the preview exists to close.

The form follows the vault rather than keeping its own numbers, so what the editor shows is what
the file holds. A form that survived its own save would be a second source of truth, which is
the thing the Launcher is not (it is an editor over the vault, never a parallel store).
