# ADR-0019: The Projection Pipeline (Renderers)

- Status: Accepted
- Date: 2026-07-26
- Depends on: [[0003-engine-presentation-separation]], [[0005-capability-first-architecture]], [[0016-asset-resolution]], [[0017-character-archetypes-world-packs]]
- Amends: [[0016-asset-resolution]] (a World declares **renderers**, not assets)
- Related architecture: [[World Packs]], [[../Build/Build From Life]]
- Horizon: renderer declaration + the `shape` renderer **IMPLEMENT NOW**; further renderer kinds + World capabilities **DESIGN NOW**; World inheritance + 3D **VISION**

## Context — the evidence

ADR-0016 made assets replaceable and ADR-0017 made identity replaceable. Step 2.5a then shipped `ui/src/components/Building.tsx`, which contains, in React:

> a `switch` on the place concept that draws a dome for `research_lab`, columns and a pediment for `knowledge_center`, battlements for `command_center`, a tower for `automation_hub`.

**The renderer knows what a Laboratory looks like.** That is the same class of violation as the engine knowing a character's name — it just moved one layer out. Changing the world's appearance today means editing React. The implementation revealed the gap; this amends the architecture in response.

## Decision

Rendering becomes **another projection layer**, exactly as the UI already is.

```
Reality (Engine)  ->  Canonical Concepts  ->  World  ->  Projection Pipeline  ->  Renderer  ->  The World
```

Three ownerships, stated plainly:

- **The engine owns meaning.** It knows concepts and nothing else.
- **The World owns appearance.** It declares how each concept is represented.
- **The renderer owns visualization.** It knows how to draw a representation, never which concept it is drawing.

### A World declares renderers, not sprites

For every concept it covers, a World declares a **renderer kind** and its parameters:

```
research_lab      -> renderer = "shape",  params = { ... }
knowledge_center  -> renderer = "sprite", params = { ... }
tile.forest       -> renderer = "shape",  params = { ... }
```

The renderer kind is part of the concept vocabulary, so a World can say `sprite` in terms the engine recognises without the engine knowing what a sprite is.

**Changing a building's appearance is editing a World. No Rust. No React. No engine change.**

### Renderer kinds

Declarable from day one, so no World manifest becomes invalid later:

`shape` · `sprite` · `animated_sprite` · `tilemap` · `vector` · `procedural` · `model3d`

**Phase 1 implements exactly one: `shape`** — authored vector primitives (polygons, rects, arcs, layers) with colour roles. It is enough to give every place a distinct, meaningful silhouette while the official art does not exist, and it removes all appearance from code.

A declared-but-unimplemented renderer resolves through the existing fallback chain (ADR-0016) to the next World, then to the default World, then to a visible placeholder. **A World asking for a renderer we do not have degrades; it never crashes.**

### Everything visual belongs to the World

Buildings · characters · terrain · roads · rivers · bridges · forests · mountains · decorations · particles · weather · lighting · UI elements · sound · music.

If replacing a World's entire visual identity requires changing Rust or React, we have not gone far enough.

## Why this is best

- Internal First, applied to rendering. The last place where meaning and appearance were fused is separated.
- One implemented renderer keeps the cost proportional to what exists (Earn Complexity) while the *contract* leaves every future technology open — including ones we have not thought of.
- Makes Epoch a platform: the same engine hosts unlimited worlds without ever knowing what any of them look like.

## Alternatives considered

- **Keep silhouettes in React** — appearance changes require code. Rejected on the evidence above.
- **Worlds ship sprites only** — locks the platform to one rendering technology forever, and forbids procedural or 3D worlds. Rejected: a World provides *renderers*, not assets.
- **Implement several renderer kinds now** — with one World, five buildings and no art, this is a cathedral. Deferred, with the contract in place so it costs nothing to add later.
- **Promote World inheritance now** — ADR-0016 already carries it as a VISION open question. We have one World; there is no evidence yet that duplication hurts. Stays VISION.

## Consequences

- `Building.tsx` loses its `switch`; the silhouettes move into the default World as `shape` data.
- The World manifest grows a renderer section per concept.
- The UI gains one generic `shape` renderer that draws whatever it is handed.
- Terrain, roads and characters follow the same path — appearance is data.

## Assumptions

- A `shape` vocabulary of primitives is expressive enough to distinguish places meaningfully before real art exists.
- Renderer kinds can be added additively, like every other concept vocabulary.

## Risks

- **`shape` becoming a general-purpose drawing language.** *Mitigation:* it stays a small primitive set. When it strains, that is evidence for `sprite`, not for extending `shape`.
- **Renderer proliferation.** *Mitigation:* a new kind requires a World that actually needs it.
- **Appearance data becoming unreadable.** *Mitigation:* authored by hand today; if that stops being viable, that is evidence for tooling.

## Open questions

- Colour roles: does the World declare a palette the renderers reference, or colours per shape?
- Where character appearance lives once sprites exist (the same renderer path, presumably).
- World capabilities (`supports weather / lighting / interiors / day-night / 3D`) — the natural extension of ADR-0016's coverage Profile, **DESIGN NOW**.
- World inheritance chains — **VISION**, see ADR-0016.
