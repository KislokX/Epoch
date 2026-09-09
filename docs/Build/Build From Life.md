# Build From Life — the Build Phase implementation philosophy

> Adopted 2026-07-25, at Foundation Complete. Governs the whole Build Phase, not one milestone.
> Architecture constitutions: [[../../EXPERIENCE_CONSTITUTION]] · [[../../LIVING_WORLD_DESIGN_GUIDE]] · [[../../CHARACTER_BIBLE]] · [[../../CONTENT_PHILOSOPHY]] · [[../Architecture/_index]]

---

## The philosophy

**We are not implementing backend features. We are implementing life.**

The engine exists to make the world believable.
The world exists to make the engine understandable.

Every milestone must satisfy **both** conditions:

1. It validates another architectural contract.
2. It makes the world **visibly** more alive.

A milestone that only satisfies the first is **not finished**. Every completed milestone should make Epoch feel more believable than the previous one.

---

## 1. The very first frame matters

The first thing a user sees defines the emotional tone of the entire application.

The app must never feel like it is *opening*. It must feel like the user is *entering a place*.

- **No generic loading screen. No dashboard. No empty chat.**
- Even when the world is almost empty, the first frame is already the World.
- If the engine is not ready yet, the World renders and fills in — it never shows a spinner in front of the world.

The user should immediately understand *"I'm entering a place"*, never *"I'm opening software."*

## 2. Idle is behavior, not animation

Characters must never feel frozen. Even with no active Quest they appear alive: reading, looking around, writing notes, stretching, walking a few steps, observing the environment.

The objective is not to animate sprites. **The objective is presence.** Users should feel the characters exist even when nothing important is happening.

Idle behaviors are **authored data** in `PresenceProfile` ([[../Architecture/World Simulation]]), interpreted by the simulation — not hardcoded animation loops.

## 3. Idle life is honest life

Routine-driven idle behavior and routine-driven movement are **legitimate under the causality rule** (ADR-0018): they trace to a declared routine rule, and they convey true information — where a character is, and that they are available.

**The hard line:** idle-class behavior must be visually distinguishable from work-class behavior. A character pottering about their lab must not look like a character executing a Quest. If there is no real work, the world must not imply there is.

Never fabricate work, conversations, or knowledge to seem alive.

## 4. Optimize for delight, not only correctness

When two implementation orders are architecturally equivalent, **prefer the one that makes the world feel alive sooner.**

Watching a character walk across the world teaches a user more about Epoch than any invisible subsystem ever will. The world is our primary interface.

## 5. World Packs from day one

Even with placeholder sprites, temporary music, free assets or simple pixel art, **the rendering pipeline must already behave exactly as it will in the final product.**

The engine references only canonical concepts — character archetypes, place concepts, animation concepts, moods. Everything resolves through the active World Pack ([[../Architecture/World Packs]], ADR-0016/0017).

When the [[../Milestones/Official Epoch Universe]] is eventually created, **only the World Pack is replaced. The engine is untouched.**

## 6. Every step is visible

Implementation order prioritises **visible life over invisible infrastructure**, whenever that does not compromise the architecture.

Each step in a milestone roadmap must state what the user can see when it is done. A step with no visible outcome must justify itself as unavoidable groundwork — and there should be almost none.

---

## 7. Reality over intention

Implementation is allowed to challenge the architecture. **Architecture is not allowed to ignore implementation.**

When implementation reveals that an architectural assumption is wrong, **do not modify the architecture immediately**. Instead:

1. **Preserve the evidence** — record what actually happened.
2. **Explain why the assumption failed.**
3. **Evaluate:** is this an *implementation* issue or an *architectural* issue?
4. **Only then** decide whether an ADR amendment is justified.

Every amendment after Foundation Complete must cite what the build revealed — never speculation.

## 8. The engine should never block the world

Build the project as **one living system**. The world, the engine, the UI and the tooling evolve together. No layer permanently waits for another.

Wherever possible the visual experience continues progressing independently of engine implementation, and the engine progressively replaces temporary implementations **without requiring architectural changes**.

**Mock policy.** A mock is acceptable **only** when it validates a contract or a user experience before the real implementation exists. As soon as the engine can provide the same contract, **the mock disappears**. Mocks are never load-bearing.

> **Build the real interfaces first. Replace the implementation later.**

## 9. A world whose engine gradually becomes real

**We are not building an engine first and then a world. We are building a world whose engine gradually becomes real.**

Every milestone should make Epoch feel more alive *while* replacing temporary implementations with real ones. The user should never feel the world is waiting for the engine — the engine quietly catches up to the world.

## 10. BUILD is the system; the vault is the reasoning

All implementation work lives in the top-level **`BUILD/`** folder: source code, Cargo workspace, React app, Tauri project, assets, World Packs, build scripts, configuration, tooling, tests, temporary mocks, development utilities.

The vault continues to document the system: architecture, ADRs, decisions, and why.

- **`BUILD/` reflects the implementation.**
- **The vault reflects the architecture and the decisions behind it.**

Keep both synchronised; **never mix implementation code into architectural documentation.** Someone opening `BUILD/` should immediately find the product. Someone opening the vault should immediately understand why the product was built that way.

## 11. Real structure over placeholder scaffolding

A directory exists because something meaningful already lives there, or because the **current** milestone is about to use it. No empty scaffolding created merely to mirror a future architecture.

**Earn Complexity applies during implementation, not only during architecture.**

## 12. Always buildable

The repository stays in a buildable state at all times. Never leave the project intentionally broken between commits.

When a milestone needs several steps, each intermediate step should still compile and run whenever reasonably possible. Small commits. Small verified steps. Everything compiles. Everything runs. Nothing speculative.

## 13. Document what exists

Documentation explains the implementation **that exists**, not implementation that is planned. Avoid documenting systems that have not been built, unless an accepted architectural decision requires it.

---

## 14. Evidence beats assumptions

Implementation generates evidence. Evidence improves architecture. Architecture changes only when implementation justifies it.

If implementation consistently demonstrates a **simpler** solution that preserves our principles, prefer the simpler solution. A design defended only by "we assumed we'd need it" loses to a working simpler one.

## 15. No invisible milestones

Every milestone leaves Epoch in a **visibly more alive** state. However much internal work happens, each milestone introduces something a user can immediately experience.

A milestone whose only output is internal machinery is not a milestone — it is an unfinished part of the next one.

## 16. Every milestone ends with a retrospective

Retrospectives are based on **implementation evidence**, never impressions. Each answers:

1. Which architectural assumptions were **validated**?
2. Which turned out to be **incorrect**?
3. What became **simpler** than expected?
4. What introduced **unexpected complexity**?
5. Did any accepted ADR gain **stronger evidence**?
6. What should be **documented without changing the architecture yet**?

Recorded under [[Retrospectives/_index|docs/Build/Retrospectives/]].

## 17. The world is the primary interface

The user should rarely feel they are navigating between pages. They should feel they are moving through a living world — walking between places, entering buildings, talking to inhabitants, watching characters move, discovering new locations.

**The UI supports the world. The world does not exist inside the UI.**

Consequences for implementation:

- The world is **larger than the viewport**, and supports **panning** and **zooming**.
- The world **expands over time**. New areas are added; previous areas are not replaced.
- **Buildings are real places.** Clicking a building **transitions inside it** rather than opening a modal or routing to another page. An interior is still part of the same continuous world, and characters visibly inhabit interiors.
- The world stays **behind everything**. HUD and panels float over it; they never replace it.
- There is always a **fast path** for a user in a hurry (a command palette), because immersion must never cost productivity.

Long-term references: [[../UX/Design References]].

## 18. Environmental Storytelling

**The environment communicates information before the UI does.**

This extends rule "animation communicates before text" one step further. Wherever possible the user should understand what is happening simply by *observing the world*. Only afterwards should the UI confirm or expand it.

> **The UI explains. The world reveals.**

This is not decoration. It is information expressed through the environment:

- A worn path says people travel there often.
- An open book in the Library says someone has been researching.
- A lit furnace says someone is working.
- A chair pulled away from a desk says someone recently left.
- An open door says someone is inside.
- A workshop full of prototypes says experimentation is happening.

None of those are ornaments. Each is engine state, rendered as place.

### What this obligates

Every place must communicate its subsystem **before** the user opens it:

| Place | Should communicate | Before opening |
|---|---|---|
| Library | that knowledge exists and is growing | the Knowledge interface |
| Laboratory | that experimentation is happening | Models & Integrations |
| Guild | that there is a party, and its state | the Party builder |
| Command Center | the state of the active Era | the Project view |

**The world itself is the first dashboard.**

### The design order this imposes

When designing any new feature, ask in this order:

1. **How would this be expressed through the world?**
2. Only then: **what UI does it need?**

Asking these in the wrong order is how a living world quietly becomes a visual theme behind ordinary software.

### Geography is UX

Geography does not exist to connect places. It exists to **communicate**, and it communicates *before* the environment does:

> **The environment communicates before the UI. The geography communicates before the environment.**

The terrain is part of Epoch's language:

| Geography | Communicates |
|---|---|
| Distance | importance |
| Isolation | focus |
| Forest | mystery, the less-travelled |
| Mountain | difficulty |
| Crossroads | collaboration |
| Bridge | connection |
| Empty land | future growth |

Moving through the world should already teach the user something **before they arrive anywhere**.

### Think world, not screen

The map is a **world**, not a canvas. Even with only a handful of places, the architecture assumes the world will become far larger than the viewport.

The camera moves **through a world**, not over a canvas. That distinction is subtle and it decides many later choices:

- Positions are in **world units**, never pixels. Pixels belong to the viewport; world units belong to the place.
- The authored world is deliberately **larger than what is currently occupied** — the empty land is where cities go.
- Future cities, forests, mountains, rivers, roads and landmarks are **extensions of the same world**, never new screens added to an application.

### The honesty constraint still binds

Environmental detail is engine state made visible — never invention. An open book means knowledge was actually written. A lit furnace means work is actually running. If nothing happened, the room is tidy and the furnace is cold, and that is *true information*.

The world may be quiet. It may never lie.

## 19. Everything visual is replaceable — the engine hosts worlds

**The engine owns meaning. The World owns appearance. The renderer owns visualization.** ([[../ADR/0019-projection-pipeline]])

Rendering is a projection layer, exactly as the UI is:

```
Reality (Engine) -> Canonical Concepts -> World -> Projection Pipeline -> Renderer -> The World
```

A World does not provide sprites. It provides **renderers**: for each concept it declares *how* that concept is visualized, and the engine never learns whether the answer is a sprite, a tilemap, a vector, something procedural or a 3D model.

**The test:** if changing a World's entire visual identity requires touching Rust or React, we have not gone far enough. Only the active World should change.

This applies to buildings, characters, terrain, roads, rivers, bridges, forests, mountains, decorations, particles, weather, lighting, UI elements, sound and music.

### The layered ownership of appearance

```
The Engine owns Concepts
The World owns Places
Places are composed from Renderables
Renderables reference Assets
The Renderer visualizes the composition
```

Each layer answers exactly one question and nothing else (ADR-0021).

**The final visual unit of a World is a Place, and a Place is a composition** — not a sprite, not a PNG, not a single Renderable. A Laboratory is a building, a shadow, an entrance, where characters stand, where the camera looks, what can be heard there.

So the same concept can be a Cyberpunk Laboratory, a Medieval Laboratory or a Minimal Laboratory. **The meaning never changes. Only the composition does.**

Ambient sound, decorations, particles, weather interaction, spawn points, interiors, lighting and camera behaviour are all *additional layers of a Place*. None of them may require changing the simulation.

**Creators are not replacing a sprite. They are authoring a Place.**

### The engine hosts worlds

The same engine must be able to host an unlimited number of completely different worlds **without ever knowing what any of them look like**. That is what makes Epoch a platform rather than an application.

### Pixel art is a direction, not a limitation

Original pixel art is the artistic direction of the **official** World, because it reinforces that Epoch is a living world rather than a desktop application. The architecture must remain capable of hosting entirely different artistic directions — and must never assume pixels.

### The application asks what, never how

The application must never ask itself *"how do I render this file?"* It asks **"what does this represent in this World?"** Everything else emerges from that answer (ADR-0020).

So the creator-facing action is **"Import into World…"**, never *"Import Asset…"*. Creators do not think in assets; they think *"this is now my Laboratory"*. The engine translates meaning into Assets, Renderables and World configuration.

**Creators are not replacing files. They are teaching the World what things are.** They are not customising Epoch — they are authoring a World.

The Asset Pipeline is not a way to load graphics. It is where the **Creator Experience** begins.

### Users travel; they do not install

Technically these are World Packs. Users should never think about installing one. They **travel to another World**.

> Not *"Install Pack"* — **"Explore Worlds"**.

Everything reinforces the same metaphor: travel, places, guilds, libraries, quests. The user is **visiting** Epoch, not operating it.

## 20. The World is the application

Not *an application that displays a World*. **The World is the application.**

The World must never become a decorative background behind traditional software. It is the primary interaction model:

- Buildings are **navigation**.
- Characters are **processes**.
- Movement is **state**.
- Animation is **information**.
- The environment communicates **before** the UI.

The UI increasingly becomes an extension of the World rather than something floating above it.

Epoch is not a retro-themed IDE. It is a living operating system whose fundamental unit is no longer a window, a file or a panel. **It is a Place. And Places are inhabited.**

### Places are experiences, not compositions

A composition is how a Place is built (ADR-0021). An **experience** is what it is *for*:

| Place | Not just | It is where |
|---|---|---|
| Laboratory | where a model lives | **research happens** |
| Library | documentation | **knowledge is explored** |
| Guild | integrations | **cooperation happens** |
| Command Center | orchestration | **decisions are made** |

Every Place must communicate its purpose **before the user clicks it**. Someone should understand what happens inside simply by observing it. That is Environmental Storytelling doing its job.

### Optimise the renderer for authoring, not for graphics

The renderer is not optimised around rendering assets. It is optimised around **expressing Places**. Assets are one ingredient; Renderables, composition, lighting, animation, sound, characters, particles and interactions are others. All of them exist so a Place can communicate what it is.

So every implementation decision is measured against one question:

> **"Does this make the World easier to author?"**

That question outranks *"does this make the renderer more powerful?"*

### The centre of the product moved

We began thinking `Editor -> Renderer -> Assets`. The architecture now points somewhere larger:

```
World Creator -> Places -> Characters -> Activities -> Terrain -> Atmosphere
              -> Lighting -> Music -> AI Agents -> Knowledge -> Integrations -> Assets
```

**Assets are not the product.** They are one resource among many that make up a living World.

Creators should not think *"I'm replacing a PNG."* They should think *"I'm redesigning my Laboratory"* or *"I'm creating a new World."*

And users should not feel they are configuring software. **They should feel they are building a World** — and living alongside its inhabitants rather than managing them.

We are not building a World Creator today. But every architectural decision should move one step closer to it. **If, years from now, someone can build an entirely new World for Epoch without touching the Engine, we have succeeded.**

### Capabilities should manifest in the World

Functional growth should have a **visible expression** in the World. Not necessarily its own building — but visible.

Installing Ollama might raise a Laboratory. Connecting a knowledge vault might give birth to a Library. Adding a code host might establish a Harbor. Docker might build a Factory. Connecting MCP servers might populate the roads with travellers.

The exact representation does not matter; the principle does. **As the user's ecosystem grows, their World visibly evolves with it.** Nobody should have to read a list of integrations to understand what their environment contains — they should be able to *see* it.

**The honesty constraint binds here too.** If installing Ollama raises a Laboratory, that Laboratory must be *true*: when Ollama is not running, the Laboratory is dark. A building that claims a capability which is not working is the World lying.

### Places evolve; they do not appear

**Capabilities do not create new Places. Existing Places come to life.**

This resolves what was an open question about growing the place vocabulary — and it is a better answer than either option that was on the table.

The Laboratory already exists. Before any model is configured it looks **abandoned**: dark, quiet, inactive. Install Ollama and nothing new appears on the map — the Laboratory *wakes up*. Lights come on, machines run, characters begin visiting, ambient life arrives.

The Library does not spring into being when a knowledge vault is connected; it becomes alive — shelves fill, books glow, researchers start reading. The Guild does not become a Harbor when a code host is added; its **behaviour** changes — messengers arrive, new activity appears.

**ADR-0017 stays intact.** The canonical place concepts remain the five the engine knows. Only how a World expresses them changes over time.

Architecturally this is nearly free, which is the sign that it is right:

- The engine already knows whether a capability is alive (`Provider::health()`, ADR-0007).
- A Place's **vitality** is derived state, exactly like occupancy.
- The World declares how each level of vitality *looks*, as Place layers (ADR-0021).

**No new engine concepts. Only existing pieces connected.**

And it stays honest — which is what separates this from gamification. **A dark Laboratory is true**: nothing is configured. It is not a locked icon dangled at the user; it is accurate information about their environment. A Laboratory that glowed before Ollama existed would be the World lying.

The user should never feel they are unlocking map icons. **They should feel they are watching a living city evolve.**

The World does not grow by adding disconnected buildings. It grows by making existing Places feel increasingly alive.

## The Living Score

At the end of every milestone, answer one question:

> **Why does Epoch feel more alive today than it did yesterday?**

If we cannot answer it, the milestone is probably not complete.

The answer must be honest. A milestone that lays a floor rather than adding life should say so plainly — that is useful information, not a failure.

---

## Measure of progress

From Foundation Complete onward, the primary measure of progress is **no longer architectural completeness**.

It is a living world that grows more capable with every milestone.

---

## Milestone acceptance checklist

Before a milestone is called done:

1. Which architectural contract did it validate?
2. What is visibly more alive than before?
3. Does every visual trace to real engine state? ([[../../LIVING_WORLD_DESIGN_GUIDE]] checklist)
4. Did the engine reference any concrete asset, filename, character name or place name? (Must be **no**.)
5. Would a first-time user understand what they are seeing in under a minute?
