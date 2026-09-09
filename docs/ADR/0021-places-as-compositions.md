# ADR-0021: Places are Compositions

- Status: Accepted
- Date: 2026-07-26
- Depends on: [[0016-asset-resolution]], [[0017-character-archetypes-world-packs]], [[0019-projection-pipeline]], [[0020-asset-pipeline]]
- Amends: [[0019-projection-pipeline]], [[0020-asset-pipeline]] (a concept maps to a **Place**, not to a single Renderable)
- Related architecture: [[World Packs]], [[../Build/Build From Life]]
- Horizon: Place composition + the layer roles the renderer currently hardcodes **IMPLEMENT NOW**; remaining roles declarable, unimplemented **DESIGN NOW**; interiors, particles, lighting, ambient sound, weather interaction **VISION**


> **Amended 2026-08-05 by [ADR-0028](0028-the-map-belongs-to-the-user.md).** This ADR describes
> `PlaceId` as a Place's identity alongside `PlaceConcept` as its classification. The build never
> received that split: `PlaceId` was implemented in the Engine as a rendering handle, and
> `PresenceState::place` and `PresenceProfile::home` were keyed on `PlaceConcept` — the
> classification doing the work of the identity, which is the defect ADR-0023 named for
> characters. 0028 finishes it, and makes a Place's concept optional.

## Context — the evidence

ADR-0019 moved appearance out of code; ADR-0020 moved it into Assets. Both treated a concept as mapping to **one** Renderable. The current renderer shows why that is not enough — `ui/src/components/WorldMap.tsx` contains, in React:

```jsx
<ellipse className="site__shadow" rx={w * 0.62} ry={w * 0.17} />   // the shadow
transform={`translate(${w * 0.66 + offset} 0)`}                    // where a character stands
<text className="site__label" y={w * 0.42}>                        // where the name sits
```

**The shadow, the character spawn point and the label position live in the renderer.** That is the same leak ADR-0019 and ADR-0020 closed, one level up: the renderer knows the *composition* of a place, not just its picture.

It also shows up as a symptom: the Researcher's activity label overlaps her building. The correct fix is not a better magic number in React — it is letting the World decide where things sit.

## Decision

**The final visual unit of a World is not an Asset, and not a Renderable. It is a Place — and a Place is a composition.**

### Terminology, stated precisely

Two things share the word *place*, and they belong to different layers:

- **Place concept** — engine vocabulary (`research_lab`, `knowledge_center`). The engine knows only these (ADR-0017).
- **Place** — a World's composition that gives a concept form. World data. The engine never interprets it.

### The layered ownership

```
The Engine owns Concepts
The World owns Places
Places are composed from Renderables
Renderables reference Assets
The Renderer visualizes the composition
```

Each layer answers exactly one question. Nothing else.

### A Place is an ordered list of layers

Each layer has a **role** and its parameters. A Laboratory is not a sprite; it is a building, plus a shadow, plus an entrance, plus where characters stand, plus where the camera looks, plus what can be heard there.

**Roles implemented now** — precisely the ones the renderer currently hardcodes, so this decision *removes* code rather than adding capability:

- `visual` — a Renderable (ADR-0019/0020). A Place may have several, in draw order.
- `shadow` — the contact shadow that grounds it.
- `spawn` — where inhabitants stand when they are here.
- `label` — where the Place's name sits.

**Roles declarable now, unimplemented** — so no World manifest becomes invalid later, and adding them is content work:

`roof` · `entrance` · `decoration` · `particles` · `light` · `ambient_sound` · `interaction_bounds` · `camera_focus` · `interior_link`

An unimplemented role resolves through the existing fallback chain and is **reported, not silently dropped** (ADR-0016). Same contract as an unimplemented renderer kind.

### The same meaning, many compositions

A Cyberpunk Laboratory, a Medieval Laboratory, a Minimal Laboratory and a Cozy Laboratory are the same concept with different compositions. **The meaning never changes. Only the composition does.**

### What this buys without touching the simulation

Ambient sounds, decorations, particle systems, weather interaction, NPC spawn points, interiors, dynamic lighting and camera behaviour all become **additional layers in a Place composition**. None of them requires a change to the engine, the simulation, or the UI's structure.

## The creator's mental model

Creators should not feel they are replacing a sprite. **They are authoring a Place.** The renderer composes and displays it.

That completes the arc started in ADR-0020: the application never asks *"how do I render this file?"*, it asks *"what does this represent in this World?"* — and the answer is a Place.

## Why this is best

- Removes the last identified leak of visual knowledge into code, with evidence rather than speculation.
- Fixes the label-overlap symptom at its cause instead of tuning a constant.
- Gives future richness a home that costs nothing today: new roles are content, not engine work.
- One concern per layer — the cleanest separation we have reached for visuals.

## Alternatives considered

- **Keep one Renderable per concept and patch the renderer's constants** — leaves shadow, spawn and label knowledge in React permanently, and every new element grows the leak. Rejected on the evidence.
- **Implement all twelve roles now** — with one asset, no particles, no lights, no interiors and no sound, this is a cathedral. Rejected; the vocabulary is declarable so nothing is lost.
- **A Place as a scene graph with arbitrary nesting** — more expressive, far harder to author by hand and to reason about. Deferred until a World actually needs it.

## Consequences

- The World manifest's `[representations.<concept>]` becomes a Place declaration with layers.
- `WorldMap.tsx` loses its shadow, spawn-offset and label-position constants; it iterates a composition instead.
- The projection carries layers with roles, and marks unimplemented roles as unsupported.
- Migrating the remaining buildings becomes authoring Places, not pointing at sprites.

## Assumptions

- A flat, ordered layer list is expressive enough before interiors exist.
- The four implemented roles cover everything the renderer currently hardcodes.

## Risks

- **Role proliferation.** *Mitigation:* a new role requires a World that needs it, and the renderer must be able to draw it before it is marked supported.
- **Authoring burden.** Places are more verbose than a single asset reference. *Mitigation:* sensible defaults — a Place with one `visual` and nothing else must still render correctly. If hand-authoring strains, that is evidence for the *Import into World* tooling, not for a smaller contract.
- **`shape` and `visual` overlapping.** *Mitigation:* `visual` carries a Renderable, and `shape` is one renderer kind a Renderable may use. No new axis.

## Addendum (2026-07-26): Places evolve

**Places are not static objects. They are living experiences.**

A Place has a **vitality** — derived state, like occupancy — and a World declares how each level of it looks, using the layer roles above (`light`, `particles`, `decoration`, `ambient_sound`).

Capabilities therefore never create Places; they bring existing ones to life. An unconfigured Laboratory is dark and quiet. Install a provider and it wakes: lights, machines, visitors, ambient life. This keeps ADR-0017's five place concepts frozen — only expression changes.

It costs almost no architecture: the engine already reports capability health (`Provider::health()`, ADR-0007), vitality is derived from it, and appearance stays World data.

**It must stay honest.** A dark Laboratory is *true* — nothing is configured. It is information, not a locked icon. A Place that looked alive before its capability existed would be the World lying, which no amount of delight is worth.

Implementation horizon: **DESIGN NOW.** Vitality wiring waits until there is a real capability whose health can drive it — the Provider arriving in Step 4.

## Open questions

- Do layer coordinates stay in footprint units, or gain a Place-local space?
- Does `interior_link` point at another Place in the same World, or at a separate interior document?
- Whether `camera_focus` belongs to the Place or to the UI's camera (ADR-0018 says the engine owns reality and the UI owns animation — this may be the UI's).

## Amendment (2026-08-19): a mark that can hold frames

`RendererKind::AnimatedSprite` has been declarable since this ADR was written, and there was no
way to say what it meant. A sprite sheet is one image, and a renderer handed one with no cut can
only draw the whole strip.

A mark now carries an optional **`Frames`**: `columns · rows · count · milliseconds ·
directions`. `Direction { North, East, South, West }` joins the Kernel vocabulary as the second
half of ADR-0016's `walk.east` — the first half arrived the day before this, as `Action`
(ADR-0018 amendment).

**Four directions, because four is what the World can measure.** A walk is one leg between two
Places, and the angle of that leg is the only thing that could ever set this. Eight would be
vocabulary with nothing behind half of it.

### The cut is the Engine's; the playing is not

What crosses the wire is how many cells there are, where they are, and how long each is shown —
facts about the artwork. *Which cell is on screen at this millisecond* is per-frame and stays in
the renderer. Same line ADR-0018 drew for characters, applied to their pictures.

### A sheet with no grid is not a picture

This ADR's rule is that the unit of rejection is the field: a bad number costs a placement, a
bad role costs a mark, the World still renders. **A cut is the exception**, and the reason is
what the asset *is*. A strip drawn whole is not a degraded animation — it is a different image,
one the author never drew.

So `animated_sprite` with a broken cut, or with no cut at all, loses **the mark**, reported, and
falls back through ADR-0016's chain. That is the same argument `resolve_appearance` already
makes about a character being one mark: an invisible person beats a wrong one, because nothing
downstream would know.

An unknown direction word fails the whole block rather than dropping one row — the rows after it
would no longer line up, and a sheet walking south where it should walk east is worse than one
that says it could not be read. A cut on a **still** mark is the mild case: meaningless rather
than dangerous, so the frames are ignored and the author is told the block did nothing.

### Nothing guesses a frame duration

Every number defaults to `0`, so "not stated" and "stated wrongly" are one case and both get
told. A default frame rate would be a constant nobody measured; the pack drew the animation and
knows how fast it runs. `count` is the one exception, and only downward: `0` resolves to every
cell, because a full sheet saying so twice is how two numbers start disagreeing.

### `is_implemented` is untouched

The renderer that plays these arrives with its own step. Until then a World declaring an
animated sprite gets `supported: false` and a visible placeholder — which is the honest reading
for an instrument with nothing behind it yet, and it is what the wire test asserts.
