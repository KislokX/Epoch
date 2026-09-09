# Experience Constitution

> A design constitution — **not** an ADR.
>
> It answers one question: **how should Epoch feel?**
>
> Every user-facing decision — UI, UX, animation, interaction, pacing, sound, workflow — is
> evaluated against this document before implementation begins.
>
> Supersedes the former `EXPERIENCE_MANIFESTO.md`, `WORLD_EXPERIENCE.md` and
> `EXPERIENCE_PHILOSOPHY.md` (archived in `docs/History/`), which answered this same question
> in three places.
>
> Where a capability belongs: `PRODUCT_ARCHITECTURE.md`. What the Experience Layer may
> actually do: ADR-0022. This changes the *soul*, never the skeleton.

---

## I. Epoch is a living world

We are not building another AI application. We are building a world powered by AI.

Launching Epoch should never feel like opening software. It should feel like entering your own
world — one that already exists, keeps evolving, where characters keep working, knowledge
keeps growing and projects keep progressing.

**The user does not start the world. The user joins it.**

People should never say *"I opened Epoch."* They should feel: **"I entered my world."**

## II. Time is a user interface

Most software shows only the current state. Epoch shows **time** — past, present, future.

Users should naturally understand what happened, what is happening and what comes next.
History is visible. Current work is alive. Future plans feel tangible.

Time is not metadata. Time is part of the interface.

## III. The world has history

Nothing feels temporary. Projects, characters and knowledge all have history. The world
remembers. When a user returns after days or weeks, the world communicates what happened — not
through notifications, but through itself.

On return, Epoch does not say *"Welcome back."* It says **"the world has continued in your
absence,"** and then narrates what actually happened:

```
While you were away...

the Researcher discovered a new architecture.
the Guardian resolved two execution failures.
the Historian documented three ADRs.
The Library received four new Knowledge Objects.
A Quest is ready to review.
```

Not notifications. The **story** of what happened while the user was gone — and only if it
happened.

## IV. The world is the home screen

There is no Home. There is only the World: the permanent navigation hub. Every launch returns
the user to it.

## V. Places instead of windows

The world is not decoration — the world **is** navigation. Places represent capabilities. The
user explores places, not windows, sidebars or dashboards.

| Place | Subsystem |
|---|---|
| the Library | Knowledge Engine |
| the Research Lab | Models & Integrations |
| the Guild | Party Builder (Definitions) |
| the Command Center | the active Era (Project) |
| the Automation Hub | Automations (Quests) |

## VI. Features belong to Places

Whenever functionality can be experienced through a Place, prefer that over a floating window
or a disconnected panel. Places are where capabilities become experiences.

"Whenever possible" is not "always" — the routing question in `PRODUCT_ARCHITECTURE.md`
decides, and administration that would be worse as a building belongs to the Launcher.

## VII. Productivity hidden inside adventure

Outside feels like a handcrafted world. Inside a Place is a professional workspace. The
transition is natural: the world is the navigation, the Place is the tool.

Immersion must never cost productivity.

## VIII. Nothing visual is decoration

If something exists visually, it exists because it has architectural meaning. Every building is
a subsystem. Every character is a runtime entity. Every animation communicates information.
Every location means something.

## IX. Animation is observability

Internally everything is powered by the Activity Stream. Externally, Activities become life.

The Researcher discovers something → the user sees her walk to the Historian → the Historian
enters the Library → documentation appears. Instead of notifications, the world communicates
progress.

Avoid spinners and abstract progress bars wherever possible. Show the work happening: the
Guardian debugging looks like the Guardian debugging. The interface explains itself through the
world's behaviour.

## X. The engine drives the world, or the world is a lie

Animations are never fake. Characters never move randomly. Everything visual originates from
the Engine. **The Activity Stream drives the world; the world visualises the Activity Stream.**

The simulation is a projection of reality, not an illusion. If nothing ran, the world does not
invent a story.

## XI. Reality always comes first

Presentation must never contradict the Engine.

Valid: provider offline → the Laboratory goes quiet → its lights dim → the Researcher waits.

Invalid: provider offline → a busy laboratory → machines running → research apparently
happening.

A dark Laboratory is **true**. It is information, not a locked icon.

## XII. Identity communicates purpose; activity communicates state

Every Place should communicate what it is *before* the user interacts with it — not only
visually but atmospherically.

- **Research Lab** — subtle machinery, electrical ambience, quiet focus
- **Library** — warm light, pages turning, silence
- **Automation Hub** — rhythmic machinery, industrial ambience

A laboratory hum when nothing is running is honest: it is what a laboratory *is*. A hum that
implies work in progress is not. Identity and activity are different concepts and must look
different (idle must never resemble work).

## XIII. Every character has presence

Characters are not avatars. They have location, state, goals, a current activity, a routine,
relationships, memory and identity. Users should eventually know where each one usually spends
their time — *"the Researcher is probably in the Lab."*

Characters must be **missable**: genuinely elsewhere when running real work. Absence must be
true, never staged.

## XIV. The user joins conversations already happening

Chat is not the centre; it is a **consequence** of the world. The user should rarely open an
empty conversation. They see the Researcher walking toward the Coordinator, and they join
something already in progress.

Always *"I'm entering their world"*, never *"I'm creating another empty chat."*

## XV. Visit is the universal interaction

Users should never learn several interaction models. They learn exactly one:

> **Visit Place.**

What "Visit" *means* may grow forever — camera, atmosphere, occupants, authored transitions,
NPC greetings, interactive systems. The interaction never changes, so users never relearn it.

Think in those terms deliberately: *visit the Laboratory*, not *click the Laboratory*. *Enter
the Library*, not *open documentation*. Language shapes design.

The enforceable contract is ADR-0022.

## XVI. The world progressively reveals itself

Nothing opens. No modal, no window, no second application, no separate scene. The user simply
experiences more of the same World.

There is no "outside" and "inside" — a Place has one identity and reveals more of itself. What
it may reveal is bounded by what actually exists.

## XVII. The camera tells stories

The camera is more than navigation. It guides attention, builds anticipation and communicates
importance.

**The camera moves. The World does not.** Users discover the world through movement, never by
switching applications.

## XVIII. The world has no canonical size

The visible area is only a window. A small installation feels like a village; a larger one
becomes a town; a mature ecosystem becomes a city.

The architecture may never assume a fixed world size, a fixed viewport, a fixed number of
Places, characters or capabilities. Exploration comes from curiosity, never from having hidden
the map — which is why the minimap exists: to say honestly that there is more out there.

## XIX. Growth is vitality before expansion

Installing a capability should not merely enable a function. It should enrich the world.

Connecting a knowledge base brings the Library to life. Installing a local model wakes the
Laboratory. Connecting a code host populates the Guild.

Capabilities do not create Places; they bring existing ones to life. Users should never feel
they are unlocking map icons. They should feel they are watching a living city evolve.

## XX. The UI frames the World

The UI exists and it matters. It is not the primary experience.

The **UI** provides administration, precision, statistics, configuration, notifications and
quick actions. The **World** provides exploration, observation, discovery, presence and work.

The UI supports the World and never competes with it. The World stays the visual and
experiential centre.

## XXI. The Launcher prepares; the World immerses

Two experiences, deliberately not merged — forcing them into one interface weakens both.

The Launcher is efficient and optimises for clarity. The World is immersive and optimises for
presence. Entering a World is crossing a boundary: not opening a screen, but boarding an
expedition. Leaving should feel like returning home.

## XXII. Assets are authored, never generated

Artwork is created deliberately, outside the Engine, and committed to the repository. When
implementation would benefit from new artwork, **implementation pauses and names what is
missing**:

```
Idea → Architecture → Missing Assets → Artwork → Repository → Integration
```

Architecture does not change to accommodate art, and placeholder artwork never becomes
production architecture.

Interaction happens **around** an asset — highlights, rings, particles, labels, camera, sound —
never inside it. A World's artwork may be PNG, SVG, pixel sprites or a format that does not
exist yet, and none of them can be assumed to contain editable windows, doors or lights.

## XXIII. Experience before implementation

Before proposing anything, ask:

> **What should the user feel during this moment?**

Only afterwards:

> **How should this be implemented?**

Implementation serves experience. Never the opposite.

## XXIV. Immersion and architecture evolve together

Epoch succeeds only if both keep improving:

- **Architectural correctness.**
- **Product experience.**

Technical excellence without immersion produces another productivity application. Immersion
without architectural discipline produces a system nobody can maintain. Neither may dominate.

## XXV. Emotional design

Epoch should not only optimise workflows — it should create attachment. Users develop habits,
favourite places, favourite characters, favourite routines. The world feels familiar not
because it repeats, but because it feels like **home**.

The goal is a place people genuinely enjoy returning to every day.

---

## The visual direction

Our inspiration is not pixel art, nostalgia or gamification. The goal is a **Living Pixel
World** — one that breathes, moves, changes, works and feels inhabited.

The visual language draws on the SNES era, not to imitate it but to capture the feeling of
entering a handcrafted world full of life. Inspiration is welcome; **assets are original**
(`CONTENT_PHILOSOPHY.md`).

## Why "Epoch"

An epoch is not merely a period of time — it is a turning point, where history changes.

Epoch connects different intelligences: models, providers, memories, projects, moments. The
user is not switching between AI models; they are travelling through an ecosystem of
intelligence. Every conversation leaves traces. Every project becomes part of the world's
history.

Epoch is not an AI orchestrator. It is a living world where intelligence has continuity.

## Three rules that keep this honest

1. **Vocabulary is a presentation projection.** Era, Quest, Chronicle, Party and History are
   user-facing labels. The Domain Kernel keeps canonical names. See
   `PRODUCT_ARCHITECTURE.md` for the mapping — it lives in exactly one place.
2. **The world is engine-driven or it is a lie.** "While you were away" and "characters keep
   working" must project real Activities and real Knowledge, never fabricated life.
3. **Every visual maps to something real.** No building, character, animation or place exists
   without an architectural referent.

---

## The pillars

| | Question it answers |
|---|---|
| **ADRs** | how the system works — the skeleton |
| **PRODUCT_ARCHITECTURE.md** | where a capability belongs — the map |
| **EXPERIENCE_CONSTITUTION.md** | how Epoch should feel — the soul |
| **LIVING_WORLD_DESIGN_GUIDE.md** | how the World behaves and communicates — the body |
| **CHARACTER_BIBLE.md** | who lives in the World — the heart |
| **CONTENT_PHILOSOPHY.md** | what may ship, and who owns the art — the skin |

---

Epoch is not software.

Epoch is a living world. A place where intelligence collaborates, knowledge persists, time has
meaning — and every time you return, the world has another story to tell.
