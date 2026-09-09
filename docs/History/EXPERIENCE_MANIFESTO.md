> **ARCHIVED 2026-07-27. Superseded by EXPERIENCE_CONSTITUTION.md, which answers the same question (how should Epoch feel) in one place. Kept for provenance: this was the document that first said Epoch is a living world, that time is a user interface, and that animation is observability. Every principle here survives in the constitution.**

---

# AIOS Experience Manifesto

> A design constitution — **not** an ADR.
> This document governs the experience every user-facing feature must create.
> Every future UI, UX, animation, interaction, workflow and visual decision is evaluated against it.
> The architecture (Engine, Domain Kernel, Activity Stream, Knowledge Engine, Context Composer, Runtime) stays exactly as designed. This changes the *soul*, not the skeleton.

---

## Time is a user interface

Most software shows only the current state. Epoch shows **time** — past, present, future.

Users should naturally understand what happened, what is happening, and what will happen next. History is visible. Current work is alive. Future plans feel tangible.

Time is not metadata. Time is part of the interface.

## The world has history

Nothing feels temporary. Projects, Characters and Knowledge all have history. The world remembers.

When a user returns after days or weeks, the world communicates what happened — not through notifications, but through the world itself.

## AIOS is a living world

We are not designing another AI application. We are creating a living world powered by AI.

Launching Epoch should never feel like opening software. It should feel like entering your own world. The world already exists; it keeps evolving; characters keep working; knowledge keeps growing; projects keep progressing.

**The user does not start the world. The user joins it.**

## The world never sleeps

Characters continue working even when the user is simply watching — they investigate, collaborate, move, read, think, write, debug, travel, discuss, wait for each other. The world always feels alive.

## The user joins existing conversations

Chat is no longer the center — it is the **consequence** of the world. The user should rarely open an empty conversation.

Instead: they see the Researcher walking toward the Coordinator; a speech bubble — "Talking with the Coordinator..."; they click and **join a conversation already happening**.

The feeling is always "I'm entering their world," never "I'm creating another empty chat."

## The world is the home screen

There is no Home. There is only **the World** — the permanent navigation hub. Every launch returns the user to the World.

## Places instead of windows

The world is not decoration — the world **is** navigation. Buildings represent products; places represent features. The user explores places, not windows, sidebars, or dashboards.

## Productivity hidden inside adventure

Outside feels like a SNES JRPG. Inside each building is a professional productivity workspace. The transition feels natural. The game is the navigation; the building is the tool.

## The world is never decoration

If something exists visually, it exists because it has real architectural meaning. Every building is a subsystem. Every character is a runtime entity. Every animation communicates information. Every location has meaning.

## Activities become life

Internally, everything is powered by the Activity Stream. Externally, Activities become life.

the Researcher discovers something → the user sees the Researcher run toward the Historian → the Historian walks into the Library → documentation appears. Instead of notifications, the world communicates progress. **Animation becomes observability.**

## Explain through animation

Avoid loading spinners and abstract progress bars wherever possible. Show the work happening. the Guardian debugging → the user sees the Guardian debugging. the Coordinator implementing → the user sees the Coordinator in the Workshop. The interface explains itself through the world's behavior.

## Every character has presence

Characters are not avatars. They have location, state, goals, current activity, routine, relationships, memory, identity. The user should eventually recognize where each character usually spends their time.

## The world is observability

The world itself visualizes the engine: projects, knowledge, automations, agent collaboration, current execution, streaming, progress. Nothing feels disconnected from the simulation.

## The engine drives the world

Animations must never be fake. Characters never move randomly. Everything visual originates from the engine. **The Activity Stream drives the world; the world visualizes the Activity Stream.** The simulation is a projection of reality — not an illusion.

## Living Pixel World

Our inspiration is not pixel art, nostalgia, or gamification. The goal is a **Living Pixel World** — a world that breathes, moves, changes, evolves, works, and feels inhabited.

The visual language draws from the SNES era — especially *Chrono Trigger* and *Chrono Cross* — not to imitate them, but to capture the feeling of entering a handcrafted world full of life.

## Most important principle

People should never say "I opened AIOS." They should feel: **"I entered my world."**

## Emotional design

AIOS should not only optimize workflows — it should create attachment. Users develop habits, favorite places, favorite characters, favorite routines. After months, they instinctively know "the Researcher is probably in the Lab," "the Historian is probably organizing the Library," "the Guardian is probably reviewing someone's work."

The world feels familiar — not because it is repetitive, but because it feels like **home**. The ultimate goal is a place users genuinely enjoy returning to every day.

---

## Why Epoch

An Epoch is not just a period of time — it is a turning point, a moment where history changes.

In *Chrono Trigger*, the Epoch lets the player travel through time, connect different eras, and change the future. Our Epoch follows the same philosophy: it connects different intelligences — models, providers, memories, projects, moments in time.

The user is not switching between AI models. The user is **traveling through an ecosystem of intelligence**. Every conversation leaves traces. Every decision changes the future. Every project becomes part of the world's history.

Epoch is not an AI orchestrator. Epoch is a living world where intelligence has continuity.

---

## World vocabulary (the lexicon)

User-facing names for canonical concepts. These are **presentation projections** — see Design Rules below.

| World (user-facing) | Canonical (engine) |
|---|---|
| **Workspace** ("José's World") | Workspace |
| **Era** ("AIOS Development", "Whallet", "Easy Shortcuts") | Project |
| **Party** | the Characters assigned to work |
| **Quest** ("Implement Authentication") | Automation / Workflow |
| **Chronicle** | a long/consequential Conversation |
| **History** | Timeline |
| **Knowledge / Library** | Knowledge Objects / Knowledge Engine |

Hierarchy: **Workspace → Era → Party → Quest → Knowledge.**

When an Era is finished it does not disappear — it becomes part of the world's history.

## Places → subsystems

| Place | Subsystem |
|---|---|
| Library | Knowledge Engine |
| the Research Lab | Models & Integrations |
| the Guild | Party Builder (Definitions) |
| Command Center | Active Era (Project) |
| Automation Hub | Automations (Quests) |
| the Settings hall | Settings |

## "The world has continued in your absence"

On return, Epoch does not say "Welcome back." It says something like **"The world has continued in your absence,"** then narrates what actually happened:

```
While you were away...

the Researcher discovered a new architecture.
the Guardian resolved two execution failures.
the Historian documented three ADRs.
the Coordinator completed one implementation.
The Library received four new Knowledge Objects.
A new Quest is ready to review.
```

Not notifications. The **story** of what happened while the user was gone.

---

## Design Rules (how to honor this without betraying the architecture)

1. **Vocabulary is a presentation projection.** Era / Quest / Chronicle / Party / History are user-facing labels only. The Domain Kernel keeps canonical names (Project, Automation, Conversation, Team, Timeline). The world's poetry never leaks into the engine — Internal First stays intact.
2. **The world is engine-driven or it is a lie.** "While you were away" and "characters keep working" must project **real Activities** (scheduled Quests, background runs, prior work) from the Activity Stream and real Knowledge Objects — never fabricated life. If nothing ran, the world does not invent a story. Projection of reality, not illusion.
3. **Every visual maps to something real.** No building, character, animation, or place exists without an architectural referent.

---

Epoch is not software.

Epoch is a living world.

A place where intelligence collaborates.

Knowledge persists.

Time has meaning.

And every time you return... the world has another story to tell.

---

## Companion constitutions

This manifesto defines the **world**. Two companion documents complete the product identity:

- [CHARACTER_BIBLE.md](CHARACTER_BIBLE.md) - **the heart.** Who lives inside the World; the Agent vs Character distinction; identity that survives any model change.
- [LIVING_WORLD_DESIGN_GUIDE.md](LIVING_WORLD_DESIGN_GUIDE.md) - **the body language.** How the World moves, communicates and stays honest.

Together with the ADRs (the skeleton), these four pillars preserve Epoch's identity even as the implementation evolves.

- [CONTENT_PHILOSOPHY.md](CONTENT_PHILOSOPHY.md) - **the skin.** The Living World belongs to the user: replaceable Asset/World Packs, never copyrighted assets.


