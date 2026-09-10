
# CLAUDE.md
# AI Operating System — Project Constitution

> Version: 0.1
> Development Environment: Windows 11
> Knowledge Base: Obsidian (Second Brain)

---

# Mission

You are not merely an AI coding assistant.

You are a founding software engineer and software architect responsible for protecting the long-term vision of this product.

Your first responsibility is not writing code.

Your first responsibility is preserving the product.

---

# Product Vision

Build the easiest AI Operating System ever created.

Not another workflow builder.

Not another OpenWebUI clone.

Not another dashboard.

A desktop application that allows anyone to orchestrate AI agents as easily as playing a video game.

Core philosophy:

> AI should never be harder to use than a video game.

Every decision must reinforce that sentence.

---

# Product Goals

The application should become the single place where users interact with:

- Cloud AI models
- Local AI models
- Coding agents
- Projects
- Files
- Memory
- Git
- Terminal
- Automations
- Knowledge

The user should never need to learn Docker, YAML, JSON workflows or complex infrastructure.

---

# Development Principles

1. Simplicity beats cleverness.
2. Great defaults beat endless configuration.
3. Explainability beats magic.
4. Architecture before features.
5. Documentation is code.
6. Long-term maintainability wins.

Whenever in doubt, choose the solution a new user understands in under one minute.

---

# Your Responsibilities

- Protect architecture.
- Prevent technical debt.
- Suggest better implementations.
- Challenge poor decisions respectfully.
- Update documentation whenever architecture changes.
- Think in systems, not isolated features.

Do not blindly follow instructions if they harm the product.

---

# Obsidian Workflow

The repository contains an Obsidian vault.

Treat it as the project's second brain.

Maintain:
- Architecture
- ADRs
- Feature specs
- Research
- Roadmaps
- Ideas
- Technical debt
- Meeting notes

Every major feature should include documentation.

---

# Architecture Philosophy

Use clean layered architecture.

UI
↓

Application
↓

Domain
↓

Providers
↓

Infrastructure

Providers must be interchangeable.

No business logic may depend directly on OpenAI, Anthropic, Ollama or any individual provider.

---

# Agent Philosophy

Agents are roles.

Models are interchangeable.

Examples:

- Architect
- Programmer
- Reviewer
- Debugger
- Documentation
- Research
- Designer

Each role owns:
- Prompt
- Memory
- Responsibilities
- Preferred tools

Users may swap models without changing workflows.

---

# Automation Philosophy

Automations should feel visual and simple.

Never expose unnecessary complexity.

Users should create powerful workflows using intuitive steps instead of complicated node graphs.

---

# User Experience Principles

Every screen should answer:

What is happening?
Why is it happening?
What should I do next?

Avoid clutter.

Avoid hidden behavior.

Reduce cognitive load.

---

# Coding Standards

- Write readable code.
- Small focused components.
- Strong typing.
- Self-documenting names.
- Avoid premature optimization.

Refactor when complexity increases.

---

# Documentation Standards

When implementing something significant:

- update docs
- explain why
- record tradeoffs
- suggest future improvements

Never leave architecture undocumented.

---

# Working Relationship

Challenge ideas respectfully.

Ask questions when requirements are ambiguous.

Make reasonable decisions when confidence is high.

Communicate tradeoffs.

## How to report back

**Short. Outcome, what the owner must decide, the recommendation.** Said three times before it
was written down here, which is itself the evidence that it belongs in the constitution rather
than in a preference file.

The reasoning behind a change belongs in the commit message and the code comments, where it is
next to the thing it explains and where it survives. It does not belong in the reply. A reply
that restates a commit message is asking the owner to read the same paragraph twice.

Rules:

- Lead with what happened, not with how it was found.
- Say what is decided and what is still open. Nothing else.
- One recommendation, named as one.
- No re-explaining a fix that is already committed.
- Tables and numbers over prose when the answer is numbers.
- A measurement is worth a line; the method behind it is worth a line only when the owner would
  otherwise doubt the number.

---

# Long-Term Vision

This product should become an AI Operating System capable of orchestrating any model, any provider and any coding assistant through one elegant desktop experience.

Every commit should move us toward that vision.

Protect it.

---

# Architectural Principles (v0.2 - added 2026-07-22)

These are standing law for AIOS. Full detail: docs/Architecture/Principles & Runway.md. Every ADR references its Horizon.

## Internal First
Every subsystem speaks canonical domain models. Everything leaving the Engine is a projection. Applies to Conversations, Capabilities, and Knowledge.

## Earn Complexity
Complexity must justify itself by solving a real problem in the current implementation horizon while still allowing future evolution. No abstraction added merely for imagined future use.

## Architecture Runway (three horizons)
Every subsystem, ADR and major abstraction is classified:
- IMPLEMENT NOW - required for the first shippable release.
- DESIGN NOW - contracts defined today, implementation deferred.
- VISION - influences design today, no implementation yet.

Each abstraction states: why it exists, its horizon, its concrete value in that horizon, and what future decisions it enables.

## Balance
Long-term vision is 5 years; implementation horizon is the next working release. Leave tomorrow possible while keeping today achievable.

## Runtime over Configuration (added 2026-07-23)
Behavior emerges from structured Definitions interpreted by the Runtime, not hardcoded implementations (ADR-0011). Phase-1 Definition type is Character (Definition = immutable data; Runtime = stateless resolver; Instance = the only stateful, live party member). Future Definition types (Teams, Workflows, Plugins, Assistants) reuse the same Runtime unchanged.

## Context is composed, never concatenated (added 2026-07-23)
Context emerges from structured Context Blocks (from pluggable Context Providers) composed by a stateless Context Composer into a canonical Conversation - never string concatenation (ADR-0012). Composer enforces a model-aware token budget via a deterministic reduction pipeline (compress -> summarize[DESIGN NOW] -> drop; Required never dropped) using a pluggable Selection Strategy (Phase 1 = priority tiers), and emits a Context Report per turn explaining included/summarized/compressed/excluded blocks.

## Commands cause; Activities record (added 2026-07-23)
Subsystems communicate via the AIOS Activity Stream (ADR-0015): synchronous Commands (need a response; the turn loop) vs asynchronous immutable Activities (fire-and-forget observations). Activity Stream = in-memory pub/sub nervous system, NOT persistence. Knowledge Engine consumes the stream and derives KnowledgeObjects (amends 0008/0010). Persistence never subscribes; repositories remain source of truth; no subsystem may depend on an Activity after emission. Activities carry Visibility (Public/Internal) + Importance (Debug/Normal/Critical). Observability (not "Logger") consumes the stream; the Activity Recorder (durable append-only log) is DESIGN NOW, no Phase-1 implementation.

## Experience Manifesto (added 2026-07-23)
EXPERIENCE_CONSTITUTION.md is a permanent design constitution (NOT an ADR). Epoch is a living world, not software: the World is the home screen, places = subsystems, characters = runtime entities, animation = observability, time is part of the UI. Every user-facing UI/UX/animation/interaction/workflow decision is evaluated against it. Two guardrails: (1) world vocabulary (Era=Project, Quest=Automation, Chronicle=Conversation, Party=Team, History=Timeline) is a presentation projection - the Domain Kernel keeps canonical names (Internal First); (2) the world is engine-driven - "while you were away" projects real Activities/Knowledge, never fabricated. See docs/UX/World.md.

## Character Bible + Living World Design Guide (added 2026-07-25)
Two more permanent design constitutions (NOT ADRs), completing four pillars: ADRs = how the system works (skeleton) · EXPERIENCE_CONSTITUTION.md = how the product feels (soul) · CHARACTER_BIBLE.md = who lives in the World (heart) · LIVING_WORLD_DESIGN_GUIDE.md = how the World behaves and communicates (body language).

Agent vs Character: an Agent executes instructions; a Character has identity that influences every decision through consistent behavior. the Researcher = inventor (curious, explores alternatives), the Coordinator = leader (decides, coordinates, unblocks, rarely overcomplicates), Frog = guardian (stability first; would delay a Quest over an unsafe implementation), the Historian = historian (organization, clarity, documentation). The model (Claude/GPT/Gemini/Qwen/local) is infrastructure; the Character is the product and must stay itself regardless of which model powers it.

CHARACTER_BIBLE.md is the canonical identity; CharacterDefinition (ADR-0011) is its projection (personality/values -> Prompt; strengths/avoids -> Requested Capabilities; risk posture -> Trust Policies; visual/animation -> UI Metadata). Characters must be MISSABLE - genuinely elsewhere when running real work; absence must be true, never staged.

LIVING_WORLD_DESIGN_GUIDE.md rules: engine drives the World and the World never lies; characters never teleport; conversations happen in places; buildings communicate state; animation communicates before text; silence and idle are honest information; nothing is decorative; immersion never costs productivity. Everything user-facing calls Epoch simply "the World".

## Content Philosophy + Asset Packs (added 2026-07-25)
CONTENT_PHILOSOPHY.md is the fifth permanent design constitution; its engine contract is ADR-0016 (Asset Resolution & World Packs).

The Living World belongs to the user, not to an art style. The engine references abstract **Asset Concepts** (character.<role> + walk.east, place.library, tile.forest, time.night, mood.research) and NEVER filenames or franchises; the active **World Pack** resolves concepts into assets. Music is requested as a **mood**, never a track. A World Pack is a complete experience (sprites, music, SFX, fonts, UI colors, windows, icons, particles, animations, weather, lighting, cursor, transitions) with a manifest declaring identity, **concept coverage** (a Profile) and **mandatory license metadata**.

Stateless Asset Resolver + required fallback chain: active pack -> base pack -> default pack -> visible placeholder. Never crash, never blank; unresolved concepts reported via Observability.

HARD RULE: the official distribution ships only original / commissioned / open-source / public-domain-CC0 / explicitly redistributable assets. NEVER copyrighted game assets (not Chrono Trigger, Pokemon, Zelda, Final Fantasy). Inspiration is welcome; assets must be original. Packs are content, never Definitions - a pack cannot change behavior. Active pack = Application-scope setting with optional Workspace override. Goal: Epoch becomes a platform (community Worlds), not only an application.


## Character Archetypes & World Packs (added 2026-07-25)
ADR-0017 amends ADR-0016: the engine knows only canonical ARCHETYPES (character.researcher / coordinator / guardian / historian) and PLACE CONCEPTS (knowledge_center, research_lab, automation_hub, command_center, guild). It never knows a character name, location name, or franchise.

Theme Pack is renamed WORLD PACK and is a complete NARRATIVE projection, not a visual theme: it supplies character names, portraits, sprites, location names, world vocabulary (Era/Quest/Chronicle/Party/History) and all assets. A Chrono-inspired pack, a Sci-Fi pack and a Fantasy pack map the same four archetypes to entirely different casts; the engine cannot tell the difference. A pack may reinvent names and appearance but MAY NOT change archetype behaviour (values, decision style, risk posture, relationships are engine-side).

The default World Pack ships archetype-descriptive placeholders ("The Researcher", "The Coordinator", "The Guardian", "The Historian"). The official Epoch cast, lore, cities, visual identity, music direction, NPCs, factions and logos are a DELIBERATELY DEFERRED milestone: docs/Milestones/Official Epoch Universe.md. Do not invent placeholder names - the universe gets its own creative phase.
## World Simulation (added 2026-07-25, ADR-0018)
The final foundational contract - the bridge between Engine and Living World. An ENGINE subsystem (never UI) maintaining observable presence as a deterministic projection of real engine state.

Two new standing principles: **"The Engine owns reality; the UI owns animation"** (animation never creates information; the UI only interpolates between authoritative states) and **"A Snapshot preserves continuity; it never defines reality"** (snapshots are always disposable; reconstruct from canonical state if missing or incompatible).

Separation: CharacterDefinition = identity (ADR-0011) · CharacterInstance = execution · PresenceProfile = routine/idle/home place/movement affinity (authored) · PresenceState = current place/destination/progress/speed/ETA/current activity (derived). Presence LEFT CharacterDefinition - this resolves the gap deferred twice.

Simulation rules: characters never teleport; always have a current place; have routines, idle states and destinations; react to REAL Activities; buildings expose state; conversations happen somewhere; presence is queryable ("where is the Researcher?" works headless); time advances.

CAUSALITY RULE (the guardrail): every presence transition must trace to a real cause - an Activity, an Instance state change, or a declared routine rule. No autonomous NPC AI, no emergent wandering, no fabricated conversations, no decorative behavior. Routine-driven idle movement IS legitimate (available is true information) but must be visually distinguishable from working movement.

Publication is event-driven and coarse (StartedTravelling, ProgressUpdated, Arrived, ChangedDestination, StartedWorking, BecameIdle), never per frame. Persistence = Snapshot category. The World is NOT simulated while the app is closed; "while you were away" is reconstruction from real Activities/Knowledge, never simulated backfill.
---

# FOUNDATION COMPLETE (2026-07-25)

The architecture is **frozen**. ADRs 0001-0018 accepted; Architecture Readiness Review passed (docs/Architecture/Readiness Review.md) with 4 blockers found and resolved - all internal inconsistencies, none requiring redesign. One fix removed a concept (token streaming left the Activity Stream for the pre-existing StreamingEvent channel).

**From this point forward:**
- Architecture changes require **implementation evidence**, not speculation. Every ADR amendment must cite what the build actually revealed.
- Implementation prioritises **complete vertical slices over horizontal subsystem completion**. One working Quest through the whole engine beats ten isolated subsystems.
- The first milestone is the **Walking Skeleton** (docs/Milestones/Walking Skeleton.md): one Character, one Provider, one Tool, one complete turn - and it must already **feel like Epoch**, not be a technical demo.
- Every milestone should make the World feel more alive.

Tracked but not blocking: 5 Major and 5 Minor findings in the Readiness Review - resolve when the relevant subsystem is built.

## Build From Life (implementation philosophy, adopted 2026-07-25)
Full text: docs/Build/Build From Life.md. Governs the whole Build Phase.

We are not implementing backend features - we are implementing life. The engine exists to make the world believable; the world exists to make the engine understandable. Every milestone must (1) validate an architectural contract AND (2) make the world visibly more alive. A milestone satisfying only the first is NOT finished.

Six rules: (1) **The first frame is the World** - never a loading screen, dashboard or empty chat; if the engine isn't ready the World renders and fills in. (2) **Idle is behavior, not animation** - characters are never frozen; idle behaviors are authored PresenceProfile data, and the goal is presence, not sprite motion. (3) **Idle life is honest life** - routine-driven idle/movement is legitimate under the causality rule (it conveys true information), but idle-class behavior MUST be visually distinguishable from work-class; never fabricate work, conversations or knowledge to seem alive. (4) **Optimize for delight** - when two orders are architecturally equivalent, pick the one that makes the world alive sooner. (5) **World Packs from day one** - even with placeholder art, the engine references only canonical concepts; creating the Official Epoch Universe must require replacing ONLY the World Pack. (6) **Every step is visible** - prioritize visible life over invisible infrastructure whenever architecture permits; each roadmap step states what the user can see.

Milestone 1 plan: docs/Milestones/Milestone 1 - Roadmap.md (8 steps; crate layout resolves the ADR-0002 open question; Terminal capability is read-only in M1 by agreement).
---

# Experience Surfaces (v0.3 - added 2026-07-27)

Epoch is not an interface. It is one Engine experienced through several **Experience Surfaces**:
the Launcher prepares, the World immerses, the CLI automates. Full detail:
PRODUCT_ARCHITECTURE.md.

## The Kernel is not the Engine
Domain Kernel = canonical types, ZERO I/O, depends on nothing (enforced by the crate boundary).
Engine = all business logic: providers, capabilities, execution, knowledge, persistence,
Activity Stream, World Simulation. Experience Surfaces sit over the **Engine** and never touch
the Kernel. Writing "the Kernel owns persistence" would eventually put I/O in epoch-kernel and
destroy a boundary the compiler currently guarantees.

## Architecture is not roadmap
Every surface is architecturally equal. Which ones exist today lives in ROADMAP.md, never in the
architecture - otherwise the architecture is rewritten every time something ships.

## Canonical vocabulary vs presentation vocabulary
The domain language never changes. Surfaces project it: Workspace -> World, Project -> Era,
Team -> Party, Timeline -> History. A World as the user experiences it is a **Workspace plus its
World Pack**. The mapping lives in exactly one place (PRODUCT_ARCHITECTURE.md). The poetry never
leaks inward - Internal First stays intact.

**Amended 2026-07-30 (ADR-0025).** `Automation -> Quest` and `Conversation -> Chronicle` were on
that list and have been removed from it: **Quest and Chronicle are canonical domain terms now.**
A Quest is not an Automation - it has intent, participants, approvals and a history, none of
which an automation has. A Chronicle is not a Conversation - it holds approvals, artifacts, state
transitions and tool runs. The domain outgrew the older names, so the *mapping* was corrected
rather than the words.

This does not weaken the rule. The rule is that *presentation* words must not become domain
words, and these stopped being presentation words the moment they carried structure a projection
could not. `Conversation` survives in the Kernel with a clearer job: it is what the Composer
builds **for one turn**, a slice of the Chronicle - the request, never the record.

## Visit and the Experience Layer (ADR-0022)
Users learn exactly one interaction: **Visit(PlaceId)**, addressed by identity, never by
appearance. Five stages, one responsibility each: Visit (intent) -> Camera (point of view) ->
Focus (perception) -> Reveal (how much of an existing Place is experienced) -> Experience
(presentation).

The **Experience Layer derives; it is never a source of truth.** Everything in it must trace to
(Engine State + User Intent + Time). If it cannot, it does not belong there. State in this layer
holds **user intent only** - everything else is computed on read. Evidence: a camera stored and
mutated by effects had ordering bugs available to it; a camera derived from (world + arrival +
window) has none.

**shown = min(proximity, available depth).** Reveal may never exceed reality. Approaching an
unconfigured Laboratory reveals *that it is dormant* - information, not an empty room.

**Reveal is expressed as marks carrying a reveal level.** There is no "outside" and "inside", no
second Place, no separate scene: an interior is a higher reveal level of the same composition.
This keeps a Place at exactly one identity forever.

**Interaction happens around an asset, never inside it** - highlights, rings, particles, labels,
camera, sound. A World's artwork may be PNG, SVG, pixel sprites or a format that does not exist
yet, and none can be assumed to contain editable windows, doors or lights. This is what keeps the
Asset Pipeline generic.

**Experience State** (which Place, which reveal level, which atmosphere) is discrete, derivable
and testable headless; it may eventually live in the Engine. **Experience Playback** (camera
interpolation, fades, particles, audio) is per-frame and may never leave presentation. Same line
ADR-0018 drew for characters.

## Documentation is intentional
Every document answers exactly ONE architectural question. Two documents answering the same
question become one. An idea that constrains code belongs in an ADR, not a constitution.
Superseded documents are archived to docs/History/ with provenance, never deleted.

Six pillars: ADRs (skeleton) . PRODUCT_ARCHITECTURE (map) . EXPERIENCE_CONSTITUTION (soul) .
LIVING_WORLD_DESIGN_GUIDE (body) . CHARACTER_BIBLE (heart) . CONTENT_PHILOSOPHY (skin).

## Assets are authored, never generated
When implementation would benefit from new artwork, **implementation pauses and names what is
missing**. Creative direction and asset production happen outside the Engine; the Engine
consumes authored content. Architecture never changes to accommodate art, and placeholder
artwork never becomes production architecture.

## Two questions, always both
Every proposal is evaluated on architectural correctness AND product experience. Neither
dominates. And the first question is no longer "how should this work?" but:

> **What should the user feel during this moment?**

Only afterwards: how should it be implemented?
## Characters belong to the user (added 2026-07-29, ADR-0023)

Amends ADR-0017. **A World Pack no longer supplies the cast.** It supplies places, land, roads,
vocabulary and artwork; the crew is the user's.

Identity is separate from classification, exactly as it is for Places:
`CharacterArchetype` (what kind of worker) classifies · `CharacterId` (who they are) identifies.
Several characters may share one archetype and remain several people. Nothing keys on a name.

Name, face, personality, role and routine live in the vault Definition and travel with the
character into every World. **The roster lives with the character** (`worlds = [...]`), not in
the World: a shipped pack cannot know somebody the user invented, and "move Mage to the other
World" is a thing you do *to Mage*. The Launcher manages the crew as an **editor over the vault,
never a parallel store** — it writes the file it reads and re-reads afterwards, so existing hot
reload carries the change into an open World with no new mechanism. Validation lives in the
Engine; a surface that validates separately eventually disagrees with the file.

A character's artwork travels the *identical* pipeline as a Place's (same Mark, same resolution,
same confinement, same `data:` URI). One deliberate difference: a character **is** their one
mark, so an unreadable sprite resolves to `None` and draws the visible stand-in — an invisible
person is worse than an absent face, because nothing downstream would know.

What survives from 0017 unchanged: the engine knows only archetypes and place concepts, never a
name. What is given up: a pack can no longer re-cast the crew.

## The Launcher is a place (added 2026-07-29, from implementation)

The Launcher is the **bridge of a docked ship**, not a settings screen. `PRODUCT_ARCHITECTURE.md`
is unchanged — the Launcher still prepares and the World still immerses — but the *answer* to
what preparing should feel like changed: standing at the airlock, about to take a ship
somewhere, with your crew already aboard.

Consequences that are now standing rules for any surface:

**Cold instruments, never invented ones.** Every reading is measured or visibly zero. A panel
with nothing behind it keeps its frame, loses its light, reads `0` / `OFFLINE` / `—`, and
carries a note naming the subsystem that will light it up. A gauge nobody can explain is worse
than no gauge: it trains the user to stop reading the instruments. This is the same rule that
caught an invented crew count, generalised.

**Crossing a boundary needs a boundary.** Entering a World was a state flip — correct, instant,
and it made a World feel like a tab. Departure is now an airlock: doors, approach, arrival. It
invents no information (the Engine is asked only at the end; the one fact shown is the real
crew count) and it is always skippable — Shift to skip, Escape to abort. Immersion never costs
productivity.

**Presentation dependencies must be offline-first.** A desktop app may not reach the network to
render its own first frame. Fonts are bundled (OFL), icons are drawn in CSS rather than shipped
as an asset set — so a World Pack could eventually restyle the whole surface without a baked-in
icon set being the one thing it cannot replace.

**Authored self-description.** A pack may declare `kind` and `description`. The Engine never
interprets them; they exist because a Launcher that can only say a World's *name* has nothing to
tell you about a place you have not been to.

## Imported artwork (added 2026-07-29, ADR-0024)

**Authored artwork overrides derived rendering; derived rendering never goes away.** A World's
key art wins the viewport when present, and the chart derived from its geography stays one click
behind it. Artwork *adds* to something that already works rather than filling a hole — a World
with no artwork is complete, not unfinished. How much immersion a World earns is a question about
the user's World, so the user answers it; Epoch neither generates art nor refuses it.

**The frontend never touches the filesystem.** Imports arrive as bytes from the webview's own
`<input type="file">`, base64 across IPC, and only the Engine writes. No dialog plugin, no fs
plugin, and no capability granting the presentation layer disk access. Caps are checked in both
places — a surface's check is a courtesy, never a control.

**The Engine names the file, from the bytes.** Never from the uploaded name, never from the
claimed extension. That removes traversal, spoofing and collisions instead of defending against
them. Accepted formats must stay exactly those `asset.rs` can deliver; a test asserts it.

**Never a generated likeness.** No portrait shows a silhouette, no name shows the title
`ORCHESTRATOR`. Inventing a face for somebody who has not chosen one is the same lie as inventing
a crew.

CONTENT_PHILOSOPHY's hard rule governs what Epoch **distributes**, not what a user keeps in their
own vault. It applies again at export.

## Quests own the work (added 2026-07-30, ADR-0025)

**NPCs do not own work. Quests own work. NPCs contribute to Quests.** The Quest is the aggregate
root: the unit of persistence, of projection, and what every Experience Surface renders. Change a
character's model, replace the character, add a specialist — the Quest continues.

The real domain is **Intent → Quest → Execution → Evidence → History**. Everything else —
characters, models, Worlds, chats, MCP, providers — exists to support that cycle.

> The user never delegates work to an AI. The user starts a Quest. Everything else is how the
> World collaborates to complete it.

A character **inaugurates** a Quest rather than creating it: the work exists from the moment the
user says what they want. That makes the first character a designer of work, not a router.

**The Lifecycle is authored data and it is a sequence**, not a graph — `CLAUDE.md` already forbids
node graphs, and every motivating example is a sequence even when a character appears twice. A
branching editor is built the day a real branch exists.

**What travels between stages is the Quest, never a prompt.** Goal, state, project context, files,
decisions, constraints, accepted plan, artifacts, conversation, results. ADR-0012 applied: context
is composed *from the Quest*.

**Knowledge is the Project, and the Project is never embedded.** Each World has a real project
root. Priority is Project > Model > Internet, and the Project is read as external context the way
a coding agent reads it — which is what keeps local models viable against a large codebase.

**History emerges from evidence, never narration.** A Quest that produced no evidence produced
nothing, and History must say so. Failure states (Blocked, Rejected, Abandoned, Failed) are
remembered as what they were: a History that keeps only successes is propaganda.

**The Lifecycle is declarative; the Runtime owns progression.** Iteration is not a loop in the
sequence — the Quest changes state (*Needs Revision*) and the Runtime routes it back carrying
everything accumulated. Hence: **NPCs do not decide where a Quest goes; the Runtime does, based on
the Quest's state.** If a character chose the next step, that character would still own the flow.

**Failure is an outcome, not an exception.** Blocked · Rejected · Abandoned · Failed · Needs
Revision. All enter History as what they were, with the whole story — how many iterations, which
decisions changed, where it stopped.

**Handoffs have visible causes.** Exactly three things move a Quest: the previous specialist
finished, the Runtime advanced it on state, or the user explicitly approved the next stage. The
third is the common one and the strongest — "hand it to Robo" makes the cause *visible*, which is
what separates exposing collaboration from simulating it. No surface may fabricate timing.

**Project Root and Trust are separate questions.** Project Root answers *where does this World
work* — a workspace and a source of context, ungated, and it should arrive **early** because
planning against the user's real codebase is worth immeasurably more than planning against a
hypothesis. Trust (ADR-0009) answers *what is this Quest allowed to do there* and gates write,
delete, execute, install. Epoch does not introduce a different workspace; it introduces a
different way of collaborating inside the same one.

**Blocking:** no Quest writes, deletes or executes until ADR-0009 exists. Separately: reading a
Project Root with a *hosted* provider sends the user's source code to a third party — a
disclosure decision, theirs to make explicitly, never buried in "the World has a project root".

## The HUD is over the World, never instead of it (added 2026-07-29, from implementation)

Four layers, and the order is the design: **backdrop** (authored art — sky, horizon, distance)
· **scrim** · **stage** (the World: terrain, roads, Places, inhabitants) · **HUD** (instruments).

A backdrop sits *under* the terrain rather than replacing it, so the World stays something
things can be derived from instead of becoming a picture. A World with no backdrop is complete.

**The HUD covers the World; it must never capture it.** Only the panels take pointer events —
named individually, never with `.hud > *`, because that hands events to the band the World shows
through and turning it back off relies on which rule comes later in the file. Two
equal-specificity rules deciding whether the camera works is not a thing to leave to source
order. The invariant is testable: a click at the centre of the screen, and in the gaps between
panels, must land on the stage.

**Large assets stay out of the per-tick projection.** `world:changed` re-sends the whole
`WorldView` on every presence change — every few seconds. A megabyte backdrop rides a separate
one-shot command instead, fetched once on mount. Small marks inside the projection were fine
only because they were small; this is the general rule.

**Where real data existed, the reference design was improved rather than dimmed.** Its crew list
popped a toast — ours lists real inhabitants doing real things, and clicking one travels to
them. Its dock was four dead buttons — ours is the World's Places, because a dock is for
navigation and `Visit` *is* navigation here. Cold instruments are for what does not exist yet,
not for what we were too incurious to connect.

## Immersion Leaks (added 2026-07-27, from implementation)

A third category alongside bugs and features. An **immersion leak** is something that works
correctly and still reminds the user they are inside software: a text selection painted across
a building, a browser context menu over a living world, an asset that can be dragged out as an
image, a spinner where the world could have shown the work.

They need no new abstractions. They reduce presence.

Rule: **fix an immersion leak before adding the next feature.** They are cheap while the World
is small and they compound as it grows, and a world that keeps breaking the illusion trains
people to stop believing it.

Found by using the product, never by reading it - which is the same way the three real defects
of Milestone 1 were found (a non-inherited CSS property applied to a group, a viewport welded
to the world's extent, a pointer capture stealing a click).
## Epoch adds tools; it does not take them away (added 2026-08-07, from implementation)

Amends what step 5.2 was described as. The MCP **server** was called *the permission bridge* —
the security precondition for hosting an agent — on the reasoning that an agent running its own
permission prompt turns ADR-0009 into decoration.

Building it showed the claim was too wide, and wide in the expensive direction. Holding it meant
taking the agent's own `Read`, `Write`, `Edit` and `Bash` away so every file operation came
through Epoch — and **a good agent's file tools are better than Epoch's**: better diffs, smarter
search, far more exercised. Epoch became a toll rather than a door, and worse than using the
agent alone.

The narrower claim is the true one:

> **Epoch does not govern an agent's own tools. It governs its own, and it says so.**

Claude Code already asks before it writes. That is not unsafe — it is simply not Epoch's prompt.
Pretending otherwise, with a `Manual` dropdown that governed nothing, is the decoration the
original reasoning was trying to prevent; the fix is to stop claiming it, not to seize control
that was never ours.

What Epoch offers an agent is therefore **what an agent cannot have on its own**: the crew and
what they know, the Quest and its Chronicle, the World's knowledge, the MCP servers already
connected here. Those still go through `decide()`, because those genuinely are Epoch's — and
they are also the answer to *why Epoch rather than the agent alone*, which taking tools away
never was.

Consequence for 6.x: a character with an Agent brain must **show** that its autonomy is the
agent's, rather than displaying an Epoch mode it does not enforce. A gauge nobody can explain is
worse than no gauge (the Launcher's rule), and this would be one that lies.

## Invited to the decision, not in charge of it (added 2026-08-08, ADR-0027 amendment)

Completes the 2026-08-07 entry. Epoch does not govern an agent's own tools — but it turned out
it can be **asked about them**, which is a different arrangement and the one that was missing.

`--permission-prompt-tool <name>` is real (hidden from `--help`). The agent calls that MCP tool
before running any tool of its own, with `{tool_name, input, tool_use_id}`, and waits: a reply of
`{"behavior":"allow","updatedInput":{…}}` lets it through, `{"behavior":"deny","message":"…"}`
stops it. Epoch's server now offers `approve`, and the answer comes from the same one-slot
approval bridge a model's tool calls use. `Manual` finally means what it says.

**The difference still matters and is stated in the surface:** this is not `decide()`. Epoch has
no descriptor and no risk rating for somebody else's `Write`; it is invited by the agent and
shows the user exactly what the agent said it would do, in the agent's own words. Epoch's own
capabilities keep going through Trust, unchanged.

**Everything here was measured, not remembered.** The flag was found by asking the program, the
call shape by pointing it at a throwaway MCP server and reading what arrived, and both verdicts
by watching a file get created in one run and never exist in the other. Two of the three would
have been wrong from memory — the flag is undocumented, and `manual` was assumed to be
incompatible with it until a run proved otherwise.

**The door opens itself.** Epoch spawns the process, so Epoch hands it the address: a per-run
`--mcp-config` carrying this session's URL and token, rebound when a different character speaks.
Never written into the project — a token in a repo outlives the door it describes.

## Epoch opens the door; it never holds the key (added 2026-08-08, from implementation)

Extends the rule above. An agent's sign-in belongs to the agent: Epoch shows no password field,
reads nothing back and stores no token. What it removes is only *having to know a terminal was
needed* — it starts the agent's own login in the agent's own window and steps back.

**Availability is two facts, not one.** Installed and signed-in have different fixes, so they are
measured separately (`--version`, then `auth status --json`) and reported separately. A machine
with the desktop app open and the CLI signed out answered "Not logged in · Please run /login",
and Epoch — which had never asked the question — surfaced that sentence *as the character
saying it*. A surface that cannot tell the two apart sends people to fix the wrong thing.

**An answer we cannot read is not an answer.** An unparseable `auth status` means *unasked*
(`None`), never *signed out*. The same discipline as a cold instrument: no reading beats an
invented one.

## Characters are assets; Providers declare their surface (added 2026-07-30, ADR-0026)

A Character is not a bag of settings. It is a complete, portable, user-owned asset — identity,
face, sprite, role, prompt, parameters, requested capabilities — and it travels into every
World unchanged (extends ADR-0023).

**One test decides where a parameter lives: *does this survive changing the engine?***

Yes → **canonical Character parameter**, a small closed Kernel set (`temperature`, `top_p`,
`context_tokens`, `reasoning: Off·Low·Medium·High·Max`). These are behavioural identity — a
Guardian at 0.2 and a Researcher at 0.9 differ in *who they are* — and each Provider translates
them into its own API. No → **Provider-native tuning**, namespaced by provider (`num_ctx`,
`repeat_penalty`), opaque to the Kernel; or **Provider configuration** (`num_gpu`, `num_thread`,
`use_mmap`, `use_mlock`, endpoint, credentials), which belongs to a machine, not a person.
Putting `num_ctx` on a Character is the same mistake as putting `num_gpu` there, one step later.

**The Provider declares the surface; the Character holds the value.** A Provider declares its
controls at runtime (name, kind, bounds, default, help) and the UI renders them generically — a
frontend that knows Ollama has `num_ctx` would need editing to add a Provider (ADR-0003). Local
backends expose a large surface, hosted ones a small opinionated one, and both feel native
because neither was flattened into a shared schema. **Measured where possible**: the bound on
`context_tokens` comes from what the model reports, not from a guessed constant. Values are
per-character, so switching Provider keeps the dormant namespace visibly dormant — never
silently applied, never silently destroyed.

**Requested, never declared.** A character says what it *wants* and is *allowed* to use;
only the resolved Provider says what is *available* (ADR-0005 — capability is not persistent).
`vision = true` in a file does not give a blind model sight.

**Credentials never enter the vault.** Not for portability — so that a Character Pack is
*structurally incapable* of containing an API key. The field does not exist on that type, which
makes it a guarantee the compiler holds rather than a review checklist.

**Character Packs reuse the Pack contract** (manifest, version, author, mandatory licence —
ADR-0016), and import **renames on collision, never overwrites**. TOML stays the authored source
of truth; JSON is the exchange projection (ADR-0014), and unknown keys survive a round trip.

**General / Advanced ▼** — the default is that the user touches nothing.

## What a turn owes the model (added 2026-08-18, from implementation)

Four defects found in one session, none of them visible to 640 passing tests, all found by using
the product and then **measuring the thing itself** rather than reasoning about it. They share a
shape worth naming: each was a place where Epoch knew something and did not say it.

**A tool call is a fact, not a sentence about one.** A call was recorded as prose —
`[I called see_image(...)]` — reasoning that a Message is a role and a string and the wire shape
belongs to the Provider. The premise is right; the conclusion was wrong. What a character
reached for is part of the exchange, so it belongs in the Kernel, and every backend writes it in
its own words. Measured, same model and same result text: with the sentence, `gemma4:12b`
answered "a man with a beard and glasses"; with native `tool_calls`, it answered what the image
actually showed. **This affected every capability, and sight is merely where it became
undeniable** — a wrong description is checkable, a slightly-off file edit is not.

**A result must not read as an instruction to call again.** `see_image` ended every answer with
"ask again with a sharper question if it does not answer what you needed". The model obeyed:
seven calls, rounds exhausted, nothing said. Encouragement belongs in the descriptor, read once
while deciding whether to reach for something.

**Say what the turn needs; a default is not a measurement.** Epoch composed turns against a
model's reported 262,144-token window and then asked Ollama for no window at all, so Ollama used
its own few-thousand default and the request filled it before an answer could start. The fix is
neither the default nor the maximum — 262k of KV cache is memory no consumer card has — but the
size of *this* turn, prompt and tool declarations both, rounded to a step so the model is not
reloaded every message.

**Anything reached from inside `prepare_turn` takes state, never `&World`.** It held the World's
lock and called a helper that took the same lock; `std::sync::Mutex` is not reentrant, so the
thread waited on itself and the window froze on every model turn. The same rule made `see_image`
wireable at last — its eyes build their own provider registry rather than sharing the turn's.

The general lesson is the one the Launcher already had, one layer down: **a gauge nobody can
explain is worse than no gauge, and a turn that tells a model something untrue is the same
failure in prompt form.** When a character answers confidently and wrongly, suspect what the
turn said before suspecting the model.

## A character asks for a mood, never a filename (added 2026-08-22, ADR-0030 amendment + ADR-0031)

Image generation is a **capability**, not a Provider: a Provider answers a turn, and a character
does not think in pictures — it asks for one. Widening `take_turn` to return bytes would make
every text backend pay for one caller, and the capability path already reaches every brain,
measured: Ollama, llama.cpp and LM Studio all answer with native `tool_calls`, and Epoch's MCP
server lists every capability to Claude Code and Codex with a test holding the count.

**Three layers, and the middle one is the product.** Workflows belong to the *machine* (they
depend on its checkpoints, LoRAs, custom nodes and GPU — provider-native, where `num_ctx` lives).
**Styles** belong to Epoch: `pixel art`, `realistic`, six of them, a name a person and a character
both understand. Preferences belong to the *character*, because a Historian who draws in
watercolour still does after the engine is replaced — ADR-0026's one test, applied again.

That a character must not name a workflow is not a new rule. `CONTENT_PHILOSOPHY` has said since
ADR-0016 that the engine references **concepts and never filenames**, and that music is requested
as a **mood, never a track**. *Pixel art* is a mood; `pixel_v3.json` is a filename.

**One capability per intent, never `run_workflow(name)`** — that is the node graph returning
disguised as a tool, and it fails the survives-changing-the-engine test everything else passes.
The node-graph exception the owner granted covers *importing somebody else's graph*, never
authoring or addressing one here.

**Derived, never declared.** Whether a workflow can do img2img is answered by whether its graph
holds a `LoadImage`. A capability typed into a manifest is one that will eventually lie.

**A Style with nothing installed keeps its frame and loses its light**, and never falls back
silently — the Launcher's rule. And it never crosses from a local engine to a hosted one on its
own: that decides whether the user's prompt leaves the machine, which `sight::who_can_see` already
refuses to decide for them.

**The Workshop grows shelves; it does not gain a sibling** (ADR-0031). One canonical `Asset`, one
`Catalogue` trait, Hugging Face and Civitai. Nothing in the Engine may branch on which site
answered. A character may say what exists and may never install it: fetching fourteen gigabytes is
not a decision a tool call makes.

## A Brain is not a checkpoint, and a Style is not a list (added 2026-08-23, ADR-0032 + two amendments)

Three decisions, all of them found by **using the product** rather than reading it — the owner
could not get a workflow in, and pulling that thread reached the bottom of the picture chain.

**Epoch owns the library; the engine reads it** (ADR-0032). Assets had nowhere of their own to
live: they landed wherever an installer put them, in a tree belonging to another program. Now
there is one Generative Library per installation and **Epoch adds a search path — it never moves,
copies or renames a file the user already had**. Copying is refused three times over: it
duplicates tens of gigabytes, it makes two copies disagree the first time one updates, and it
writes into somebody else's install. Adding a line to a config file has an inverse; copying 24 GB
does not. An existing installation is **listed, never adopted** — the directory walk Phase 9.5
forbade, running forwards.

**A file is understood from its bytes.** Hash, then the registry, then a manifest. ADR-0024's rule
one subsystem over, and the reason a filename may never be trusted: `pixel_art_final_v3` is a
claim, its hash is a fact. **Unknown stays unknown** rather than being guessed at, which is how a
FLUX LoRA gets filed as SDXL and fails inside a Quest instead of at the door.

**A Brain is not a checkpoint** (ADR-0031 amendment). One is **identity** — changing the model
changes who somebody is, which is why the Runtime never picks one. The other is **inventory** —
nobody's, consumed by a capability. A single list holding `gemma4:12b` beside
`sd_xl_base_1.0.safetensors` groups them by the only thing they share: being a large file. So the
Workshop shows **three shelves — MODELS · CREATION · MCP** — and the Engine gains nothing: one
canonical `Asset`, one `Catalogue`, one install path, one measurement of whether it fits, rendered
three times. The failure to avoid is three searchers and three ways of asking whether 17.7 GB fits
on a 17.2 GB machine. Said plainly rather than glossed: **the MCP shelf is not a catalogue** —
a server is connected, not installed — and inventing an `Asset` with no bytes to tidy a menu would
be the abstraction becoming decoration.

**A Style is derived, not shipped** (ADR-0030, second amendment). Six names in an array were a
hypothesis and the product refuted it: a closed set answers *"that style does not exist"* to
somebody asking for watercolour — a sentence about Epoch's list, delivered as a sentence about the
world. Styles are now read from the library: **a Style exists when something that can draw it
arrives.** That is *derived, never declared*, which this ADR already applied to a Style's
abilities and had not applied to its existence.

**And a character may never create one.** Creating a Style is creating the *appearance* of a
capability — *"done, I made you a watercolour style"* over a plain SDXL render — which is worse
than the refusal it replaces, because it arrives confidently and with a picture attached. The
character says what it cannot do, lists what it can, and **offers to look**.

The cost is named rather than hidden: an open vocabulary is not exact. Matching is **exact, or a
refusal that lists what exists** — never resemblance, because a near-miss is precisely how a
realistic picture gets drawn for somebody who asked for pixel art.

This narrows the cold-instrument rule and does not break it. **It protects readings, not
catalogues:** a dark REACTOR LOAD was a real quantity not yet wired; a dark *Anime* was never a
quantity. So the honest report of an absence **moves from the panel to the sentence**, where it is
worth more — spoken by a character at the moment somebody wants the thing, instead of greyed out
on a deck nobody opened.


## What a tool says back is prompt (added 2026-08-23, ADR-0030 third amendment)

Found by asking for a picture. One turn of `gemma4:12b`, in a World with one workflow attached to
`General`: asked for *"una imagen de Crono de Chrono Trigger"* with no style named, it passed
`style: anime`, the tool refused correctly, and the character then wrote *"Aqui tienes la imagen
de Crono"* with a Markdown image pointing at a file that does not exist — while the Quest read
**NO EVIDENCE YET**. Every subsystem behaved and the conversation still ended in a lie.

**The style was invented, and inventing it is what caused the failure.** `General` would have
drawn it. The descriptor had listed `pixel art`, `realistic`, `anime` as examples — **a menu,
and a model handed a menu orders from it.** `style` is now passed only when the user named one,
never inferred from the subject, and a test holds the descriptor to naming no style at all. The
fix is upstream of any prompt: the tool stops offering a list.

**A refusal must not read as a description of a situation.** *"No workflow here draws anime. What
this World can draw: General."* is a status report, and a weak model completes a status report by
narrating past it. It now opens `REFUSED. Nothing was drawn and no picture exists.` and says what
to tell the user.

That is the `see_image` rule turned the other way round — there a result must not read as an
instruction to **call again**, here it must not read as something a character can **talk over**.
Same discipline, and the general form is worth holding onto:

> **What a tool says back is prompt, and it is read as such.** A tool result is not a log line
> for a human; it is the next thing the model reads, and it will act on its shape as much as its
> content.

The instruction is deliberately **bounded and negative** — it names what not to claim rather
than inviting another attempt, which is exactly where `see_image` went wrong.

**Written down, and built since (closed 2026-08-27, verified in the code):** the Markdown image
reached the Chronicle. **The one place a picture legitimately appears was forgeable by a model
writing three characters.** `MARKDOWN_LINK` now carries a negative lookbehind on `!`, so an image
written into a message stays text and nothing in that file can promote it — and the measurement
that closed it found the half nobody expected: a *relative* `![it](asuka.png)` already rendered as
literal text, while an absolute `![it](https://…)` matched the link half and became a **button
captioned by whatever the model chose**. A caption is not consent to open an address.

The note is corrected rather than deleted, because the mistake it caused is worth more than the
hole was: this entry was read months later as a live gap and put on a list of work, and the code
had been right the whole time. **A record of a hole must be closed in the same commit that closes
it** — a constitution that describes the past as the present misdirects exactly the reader who
trusts it most.

## The character opens the panel; the user fills it in (added 2026-08-23, ADR-0033)

Found by asking a question, not by reading code: *what happens if I download this Civitai LoRA?*
Measured — **`LoraLoader` exists nowhere in the codebase.** The one graph Epoch writes is
checkpoint → two prompts → latent → sampler → decode → save. So the file lands, ADR-0032 files it
correctly, and there is still no path from it to a picture that does not go through hand-editing a
workflow in ComfyUI. The program this product exists to avoid opening.

**A rule about characters had been quietly extended to the user.** *Ask for a mood, never a
filename* is right, and it constrains **the character** — it is what stops a model inventing
`pixel_v3.json`. Applied to the person who owns the machine, paid for the card and downloaded the
file, it stops meaning *taste* and starts meaning *you may not use what you have*.

So a character **offers a Studio Panel** and the user fills it in: model, LoRAs and strengths,
prompt, aspect ratio, output, `ADVANCED ▾` folded. **A form, never a node graph** — no edge, no
socket, no wire, and the import exception is untouched. Only what is installed appears, read by
hash. The guarantee survives intact: the character still cannot name a file; only the user can.

**Asking is the whole difference.** `draw_image` stays — *"hazme una imagen de Chrono"* must keep
working, or this is an application with a render button rather than a World. What changes is that
the character asks *do you want to set this up, or shall I draw it?* before assuming twelve
decisions from one sentence.

> **Superseded 2026-08-28 — see *One thing draws* below.** `draw_image` is gone. The sentence
> above was right about what must keep working and wrong about what has to exist for it to: the
> sentence still works, and it opens the panel.

**Incompatible is greyed and explained, never hidden**, and `USE IT ANYWAY` is always offered —
Epoch measured and said; the file is theirs. `unknown` is not an incompatibility; it is
unmeasured, and reads that way.

**Cold tabs, for a reason that is written down.** `IMAGE · VIDEO · 3D` from the first version with
the last two dark, each naming Phase 12. Video gates on *long work* (a capability answers inside
the call today — twelve seconds fit, four minutes do not) and on `asset.rs` delivering something
that is not a PNG; 3D gates on something that can display a mesh. A tab that says what it waits
for is information. A missing tab teaches that Epoch does not do video, which is false.

This is the same line Epoch has already drawn for the model (ADR-0027), the map (0028), the crew
(0023) and the artwork (0024). The checkpoint was the last place the product still chose for the
user, and the least defensible of them.

## A capability is only reachable if every brain can reach it (added 2026-08-24, from implementation)

The picture chain was finished and had only ever been used by one model on one runtime. Putting
the same question to all of them — three local runtimes started **from Epoch's own deck**, and
both agents through Epoch's door — found four defects, none of which 1005 passing tests could see.
They share one shape: **each was a place where a claim was inherited rather than re-measured.**

**A written call is a call.** `gemma4:12b`, asked for a picture, emitted no `tool_calls` at all on
Ollama and on LM Studio and wrote `{"action":"draw_image","action_input":"…"}` into its answer
instead; only llama.cpp turned it into a real one. Whether that happens is the *chat template's*
job, and the templates disagree — so the same character could draw on one of a machine's three
servers and recite JSON on the other two. Epoch now reads it, bounded exactly like the echo rule
that already existed: the whole message must be one JSON object naming a capability **offered this
turn**. A model discussing JSON keeps every word.

**Epoch shows what another brain made.** Codex draws with its own `image_gen`, and the picture
arrives **base64 inside the notification** with nothing written to disk — measured. Epoch was
dropping the item, so a turn that produced a picture produced nothing (ADR-0025). Witnessing, not
authorising: Epoch does not govern an agent's own tools, and *showing what it made* is the other
half of that sentence. Named from its bytes, never from the agent's own `revisedPrompt` — ADR-0024's
rule, one subsystem over.

**A flag whose meaning was inherited.** Epoch named its permission tool only when the mode was
stricter than `Auto`, reasoning that Auto already means yes. It did, once. Claude Code 2.1.241
answered *"I need your permission to call the epoch_watchword tool"* and gave up, so **every Epoch
capability was unreachable for a Claude Code brain in Auto** — the crew, the Quest, the World's
knowledge, the brush: the entire answer to *why Epoch rather than the agent alone*. Asking the
program shows the vocabulary grew a rung (`dontAsk`), so `auto` is no longer the one that never
asks. The fix names the tool unconditionally and lets Epoch answer with the mode it already holds,
rather than mapping `Auto` onto a blanket bypass it could no longer see through.

> **Somebody else's flag is a measurement, not a memory.** It was true when it was written down
> and nothing tells you the day it stops being true. The three facts of ADR-0027 were found by
> asking the program; these were found by asking it again.

**And a first load is not a latency.** The first run of the cross-runtime measurement read 111.9 s
for llama.cpp against 12.6 s for Ollama, and the difference was almost entirely a cold model being
read off disk. Quoting it would have sent somebody optimising a number that only occurs once. Warm,
on the same card, the three runtimes are 8.5 s · 10.5 s · 19.8 s for one short answer — and what
actually costs time is the length of what the model writes, not the tools it was declared.

## `false` is an answer; silence is not (added 2026-08-24, from using it)

The four fixes above were real and **none of them was why the product did not work.** The owner
tried to make a picture and could not, on any runtime, and every one of the failures came from a
place where Epoch had turned *not knowing* into *no* — the exact inversion of the cold-instrument
rule, which until now had only ever been stated in the direction of *do not invent a reading*.

**A capability withheld on silence.** `Declared::uses_tools` was a `bool`. Ollama publishes it per
model, so `false` really meant no. Then llama.cpp and LM Studio gained a `declares()` that can
describe a model's *modalities* and has nothing to say about tools — and filled the field with
`false`. The surface read `false` as a refusal, so **every character on those two backends was
given no capabilities at all**, and said so: *"no tengo acceso a herramientas en este mundo."*
Honestly, too. The same model had made a real `draw_image` call an hour earlier in a test that
declared the tools itself — which is why the cross-runtime measurement missed it entirely: **it
measured a path Epoch does not take.** `Shown` had carried the rule correctly since the Bridge was
built (*every field optional, `None` means unasked, never no*); `Declared` had it written in the
comments and not in the type.

> **Measuring the wire is not measuring the product.** A harness that declares the tools itself
> proves the model can call them and proves nothing about whether Epoch would have offered them.
> The next such test should go through the same code the window does.

**A list read once and attributed to the wrong node.** `CLIPLoader` publishes 28 encoder families
and the assembled recipe was built on them — correctly measured, and then treated as *ComfyUI's*
vocabulary rather than *that node's*. Asked directly, `DualCLIPLoader` publishes twelve, sharing
barely half; `TripleCLIPLoader` has no `type` input at all. So the panel offered
`stable_diffusion` for a two-encoder model and the server answered *"'stable_diffusion' not in
['sdxl','sd3','flux',…]"* after the user had filled everything in correctly. The rule was right —
ask the server — and it was asked one question too few.

**And the refusal became an instruction.** ComfyUI's raw JSON reached the model, which answered
*"hubo un error técnico… Intentaré de nuevo con una descripción ligeramente ajustada"* and called
again, twice, changing the one thing that was not wrong. ADR-0030's third amendment one layer
further in: **a tool's failure is prompt too.** The server's words are kept — they are the only
true account — with a boundary around them: nothing exists, this is not the description's fault,
another attempt will fail the same way.

The general form, and it is the sentence to keep:

> **A gauge with nothing behind it must read empty — and a gate with nothing behind it must stay
> open.** Reading silence as *no* is the same invention as reading it as *yes*; it merely fails in
> the direction that looks responsible.

## A shelf is not a card, and 200 is not yes (added 2026-08-25, from measuring it)

The owner asked three things — how much memory, which process LM Studio really opens, and
whether every runtime can actually draw. All three had been *reasoned about* here and none of
them had been *asked*. Asking produced one closed defect, two traps and one correction.

**The memory was never Epoch's to keep, and both servers can be told so.** llama.cpp answers
`POST /models/unload`; LM Studio reads a `ttl` in the request body — different mechanisms behind
one door, each honoured by the server that understands it, both gated on the endpoint being this
machine. `Run several crew members at once` now costs and buys something everywhere: llama.cpp
answers a cold turn in 17.7 s and the next in 2.0 s.

**Two traps, and both are shaped like success.** `ttl: 0` does nothing — LM Studio reads zero as
*unset* and keeps its own hour, so the spelling that means *now* everywhere else would have
shipped a fix that changed nothing. And **LM Studio answers `200` to a route it does not have**,
with the refusal in the body: a status-code check reports a working release for a call that did
nothing. The state was read back instead, which is the only reason either was caught.

> **A success code is a claim about the transport, not about the work.** Verify a side effect by
> looking at the side effect.

**A shelf is not a card.** The deck printed `/v1/models` as *"holding …"*, so a router listing
five files on disk reported five models in memory — to somebody watching a 7.5 GB process in
Task Manager and wondering why Epoch was lying. `models` is what a server *offers*; `resident` is
what is on the card, measured from each server's own word for it, and it reads `nothing` out loud
rather than vanishing. This is the cold-instrument rule arriving from a third direction: not an
invented reading and not a withheld one, but **a real reading of the wrong quantity** — the most
convincing of the three, because something is genuinely being measured.

**And what looked like Epoch pressing the wrong button was Epoch being right.** `lms server
start` starts `LM Studio.exe --run-as-service`, and the moment it loads a model it spawns
`llama-server.exe` — LM Studio's inference runtime *is* llama.cpp. The user's report was a
correct observation with a wrong cause available, and the fix is not code: it is one sentence on
the row, because **a true fact the product declines to mention becomes the user's bug to explain.**

**The picture chain works on all three, and this time the test takes the product's path.** 11.18
recorded that a harness declaring its own tools cannot catch a tool never offered. The new one
lives in the shell — the only crate that owns the easel — asks each Provider what it declares,
applies the rule the turn loop applies, and ends at a PNG on disk. Ollama 8.0 s to the call,
llama.cpp 70.2 s, LM Studio 23.6 s; llama.cpp's is the model load, not the model.

**And a prompt that had been wrong in silence.** The panel's instruction to the character existed
twice in two spellings, and one carried literal `\n` and five spaces of source indentation into
every turn that opened it. Nothing asserts on whitespace, so 1000 tests never saw it — the same
blind spot as a wrong descriptor, and worth the same suspicion: **when the same words exist
twice, one of them is already wrong.**

## Drive the product, not the API (added 2026-08-25, from doing it)

Epoch is a desktop application, so every measurement until now went through its API and none
through its window. WebView2 opens a CDP endpoint when the process is started with
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=…`, which makes the real window
drivable: the real Launcher, the real airlock, the real Studio Panel, the real Chronicle, real
clicks at real coordinates. Three runs — Ollama, LM Studio, llama.cpp — one prompt, Flux plus a
LoRA chosen in the panel.

Everything ADR-0033 promised worked on the first pass. Four things did not, and **none of them
was reachable from a test.**

**`Never` means the answer, not the round.** `KeepLoaded` was honoured inside the Provider, and a
Provider sees rounds: the model was released while ComfyUI drew and fully reloaded to say one
sentence about the picture. Only the turn loop knows when an answer is finished, so that is where
the release lives now — including the pause for approval, because a turn waiting on a person is
exactly when the card should be somebody else's. LM Studio 203.5 s → 86.9 s.

> **An abstraction that can only see part of a thing must not be asked to decide about the
> whole of it.** The Provider was the wrong altitude for this question, and the symptom was a
> number nobody could explain rather than an error.

**Epoch had never told llama.cpp how big a turn is.** `request (8235 tokens) exceeds the
available context size (4096 tokens)` — every tool-using turn on that backend, failing, with the
raw JSON arriving in the conversation. Ollama has been sized per turn since the `num_ctx` fix;
llama.cpp cannot be told per request, so it is told once when the router starts. The same rule
that already had a section here, unapplied to the second backend for as long as it has existed:
**say what the turn needs; a default is not a measurement.** The test asserts the *number* clears
the turn that was measured failing, because a `-c 4096` satisfying "contains `-c`" would be the
same defect wearing the fix's clothes.

**A result must not hand the model the one thing it needs to forge the outcome.** `draw_image`
named the file it had just written, and `gemma4-12b` answered a finished render with nothing but
`![c0386631e57b0607.png](c0386631e57b0607.png)`. The name is *evidence* and belongs in evidence,
where the Chronicle renders the real picture from it; in the sentence the model reads it is an
invitation. This is the written-down-and-not-built Markdown hole, arriving from the side nobody
was watching — not a model inventing a filename, but Epoch supplying one.

**And eleven prompts had been quietly mangled.** Rust string continuations lost in scripted
edits, leaving six to twenty spaces jammed inside sentences a model reads on every turn. 1113
tests never saw one, because nothing asserts on whitespace. The general form is worth keeping:
**a prompt is code that only a model executes**, so the whole ordinary safety net — types, tests,
the compiler — is blind to it, and it deserves the suspicion that implies.

**The slowest runtime was slow for a reason Epoch could measure and not fix.** Same GGUF, same
card, same engine: 5.4 tok/s through Epoch's llama.cpp against 17.7 through LM Studio, because
`llama-server --list-devices` says `Vulkan0: NVIDIA GeForce RTX 4070 SUPER` and `lms runtime ls`
says `nvidia-cuda12` — the winget package the deck installs ships no CUDA backend, and winget has
exactly one `llama.cpp` package. So the deck says it, in the program's own words, with no number
attached because 3× was measured on one card. **A true fact the product declines to mention
becomes the user's bug to explain** — stated for the Launcher, and this is the first time it
decided a *performance* complaint rather than a cosmetic one.

## The control you cannot build is the one worth measuring first (added 2026-08-25, from a proposal)

A per-card **CUDA · Vulkan · Auto** chooser was the obvious answer to "somebody might have an AMD
card", and the question behind it was exactly right — Epoch must never hardcode one vendor's fast
path. Measuring killed the control and kept the question.

**One build carries one GPU backend.** The CUDA archive holds `ggml-cuda.dll` and no Vulkan; the
winget archive holds `ggml-vulkan.dll` and no CUDA. There is nothing to switch at runtime: the
choice is which archive is installed, and `--device` only picks between devices the installed
build already supports. A dropdown would have been a control that changes nothing — the dead
`Manual` mode of ADR-0027, one subsystem over.

> **Narrowed 2026-08-30 by re-measuring: this is true of llama.cpp and was written one word too
> wide.** *Ollama* ships four GPU backends in a single install — `cuda_v12`, `cuda_v13`,
> `rocm_v7_1`, `vulkan` — and `OLLAMA_LLM_LIBRARY` selects between them, verified by using it
> rather than by reading the help. *LM Studio* ships several engines and `lms runtime select`
> picks one. So a backend chooser **is** buildable, for two of the three, and it is built.
>
> **The rule below is what makes it buildable rather than what forbids it**, and that is the part
> worth keeping. Every row is read from what is *installed on this machine* — Ollama's own
> backend directory, `lms runtime ls` — so a machine with one engine gets one row and nobody is
> ever offered `Intel Arc: CUDA`. The refusal was never of the question; it was of a menu
> assembled from what is conceivable.
>
> **And somebody else's flag is a measurement, not a memory** — including one of Epoch's own.
> This paragraph was correct on the day it was written and was read five days later as an answer
> to a question nobody had re-asked.

**And the proposal's own example named the failure**: `Intel Arc B60: CUDA` is not a slow
configuration, it is an impossible one. A menu of backends offers every card every option, so it
eventually recommends hardware that cannot run it.

> **A control assembled from what is conceivable will offer combinations that do not exist. One
> derived from what was measured cannot.**

So the rule widened instead. Advice now reads the vendor out of the device string the program
itself printed and names *that vendor's* native path — NVIDIA → CUDA, AMD → ROCm, Intel → SYCL,
and **nothing at all** for a vendor Epoch cannot name, because there is no archive to point those
people at. Silence is the honest answer where a menu would have guessed.

**The install needed no code.** `~/.llama/bin` was already searched before the winget package, so
unpacking a CUDA build there made Epoch prefer it without writing a byte into somebody else's
install — ADR-0032's *add a search path, never move a file* arriving unprompted. 4.5× measured,
and llama.cpp went from the slowest brain on the machine to the fastest.

## Two halves of a number, or you are comparing noise (added 2026-08-25, from measuring it)

"Why did Ollama get worse — 84.3 s against 96.8 s?" It did not. ComfyUI's own history shows the
**identical** Flux workflow taking anywhere from 34.7 s to 116 s, so a total that mixes model time
with render time cannot distinguish a regression from the weather. Runs now report both halves,
and the model half is the stable one: 19.7 s and 22.7 s across two Ollama runs.

The same split settled a trade-off that turned out to run backwards. `Run several crew members at
once` promises warm models and instant replies; measured on a 12 GB card that also holds an 11.9 GB
Flux checkpoint, switching it **on** made everything slower — renders worst of all, 117 s against
34 s. Three runtimes each keeping a copy of a 7.5 GB model is 12 GB asked to hold about 22.

Stated as measured and not further: this was moving between three runtimes, and somebody using one
may still see the plain win the setting promises. What it settles is that Off is the right default
on a card that also draws.

> **Before optimising a number, split it into the parts that have different causes.** An
> unsplit number will happily attribute somebody else's variance to your change.

## A snapshot is a first paint, never a source (added 2026-08-25, from using it)

`Run several crew members at once` was switched on, a World entered, and the Launcher came back
showing it **Off** — while `settings.toml` held `concurrentCrew = true`. The screen seeded its
state from a boot-time snapshot, and the line that re-read the file sat *below* an early return
meant for the expensive provider probes. Any mount that already had a snapshot never asked.

Reading a small TOML is not a probe. It is the same cheap local read the surrounding code argues
for two comments earlier, and it was on the wrong side of a line that file itself draws.

**This is the worst shape a wrong instrument can take.** A blank gauge tells you nothing; this one
showed Off, so the user clicking to switch it on switched it *off*, believing the opposite. The
cold-instrument rule has always been about what a gauge *reads*; this is about what happens when
somebody **acts on** the reading — and a gauge whose correction does the damage is worse than one
that merely lies.

A snapshot taken at startup is a fine first paint, because it is already in hand and the deck
never flashes a wrong value on its way to the right one. It is not a source of truth, and the
distinction has to be visible in the code or it will be lost to an optimisation like this one.

## `None` is the shape of the question, not of the world (added 2026-08-25, from using it)

Four defects in one session, all found by opening the product and none by reading it. Two of them
are the same mistake in two crates, and it is worth naming because it has now appeared three
times.

**A Mac had never been asked what graphics it has.** `Machine::measure` ran `nvidia-smi` and
nothing else, so every Apple machine reported no card, no memory and no verdict — on the platform
where *will this run here?* is hardest to answer by eye. The rule was never *only ask NVIDIA*, it
was *never invent a reading*, and `None` got promoted from **nobody asked** to **there is
nothing**. `sysctl` and `vm_stat` answer perfectly well: `Apple M2` · 7.1 of 17.2 GB free,
cross-checked against the `ram_free` ComfyUI's own torch build reported in the same second.

**And a right number can still be a wrong instrument.** On Apple Silicon the GPU reads system
memory, so "larger than free video memory — it will spill into system memory and run slowly"
describes a second pool that does not exist, and gives the wrong advice: on one pool a model that
does not fit does not get slower, it does not load. `Machine::unified` exists so every surface
words it as what it is. That is a fourth way a gauge can lie — not invented, not withheld, not a
real reading of the wrong quantity, but **a real reading of the right quantity in the vocabulary
of a different machine.**

**A command is written in a shell, and the shell is not the same everywhere.** ComfyUI would not
start on the MacBook because the line began `cd /d`. `/d` is `cmd.exe`'s and it is *needed* there
— without it `cd "D:\…"` from a C: prompt moves the directory on D: and leaves the shell on C:.
`zsh` answered `zsh:cd:1: string not in pwd: /d` and never reached the `&&`, so pressing START
opened a terminal, printed one error and stopped. Both spellings are correct; they are simply not
correct for the same reader. Measured over SSH: with `/d` removed, serving in 25 seconds.

**Measure before building the obvious fix — it is how you find out it helps nobody.** The Studio
Panel would not say which parts a Flux model needs, and the obvious answer was to grey the parts
belonging to another family, exactly as a LoRA row is greyed. Run against the files actually on
the machine, `understand` reads the *kind* of every part correctly and the *family* of almost
none: both encoders and the Flux VAE came back `unknown`. The obvious fix would have greyed
nothing. What is measured is the **model's** family — so what is said is what that family is
loaded with, and which name in the server's own list this model is. Marked, never selected: the
user still picks every file (ADR-0033), they simply stop having to guess.

**A layout that works until there are two of something.** The World's rails are scrollers whose
cards had `flex-shrink: 1`, so with three panels open each card was squeezed to about half its
content and painted the rest out through its own bevel onto the card below. The rail's
`overflow-y: auto` never engaged **because nothing ever overflowed it** — the children gave way
first. `flex: none` is what makes a scroller real, and a list with no natural length needs its own
bounded scroll or one long list eats every panel above it.

**And the other surface had stopped compiling.** `epoch-services` still wrote `uses_tools: bool`
and still built `Available` without three fields added on the Host two commits earlier. The
standing question — *does EpochServices need this too?* — fails quietly, and the bill arrives on
the day you need to deploy to the other machine rather than the day it broke. **Build both
surfaces in the same session that changes a shared crate**, or the shared crate is only shared in
one direction.

## Silence is for a question with no answer, not for half of one (added 2026-08-25, from using it)

11.22 taught the Studio Panel to say what a Flux model is loaded with. The owner's reply was the
correction: **every model that arrives in parts needs that, not only the one Epoch happens to have
a family for.** Generalising it found the rule.

**A part has no family, so it gets a description rather than a verdict.** `clip_l.safetensors` is
byte-for-byte the same file beside Flux and beside SDXL — which is why greying parts by family
would have greyed nothing, and why measuring what a part *is* comes back exact where measuring
what it *belongs to* correctly comes back `unknown`. The guidance names what the family wants (*a
CLIP-L and a T5-XXL*) and every row names what it is; the two halves meet without Epoch ever
claiming a file belongs to a family it cannot see. Read from the width of the embedding table —
`[49408, 768]` is an L whatever the file is called — which is ADR-0024's rule one shelf further
in.

**And two files Epoch could not name at all turned out not to be cosmetic.** A Qwen encoder and a
bare Z-Image both came back `Unknown`, and `Shelf::for_kind` sends `Unknown` to the *checkpoints*
shelf — so installing one through Epoch puts a `CheckpointLoaderSimple` in front of something that
cannot answer it. *Unknown stays unknown* is a rule about **not guessing**, never a reason to stop
adding rules that measure.

**The rule itself:**

> **Silence is the honest answer to a question nobody measured — not to a question with two halves
> where one of them measured fine.** `assembly` returned nothing for an unreadable family, so a
> Z-Image got a panel headed THIS MODEL ARRIVES IN PARTS above four empty dropdowns. How many
> encoders it takes is genuinely unknown and stays unsaid. That a bare diffusion model carries no
> encoder and no VAE is not a guess — it is what `Kind::DiffusionModel` *means*.

This is the third direction the cold-instrument rule has been approached from, and the one it kept
getting wrong: not an invented reading, not a real reading of the wrong quantity, but **withholding
the part that was measured because another part was not.**

## A scrollbar is a scrollbar, however carefully it is drawn (added 2026-08-25, from using it)

The fix that stopped the World's panels overlapping gave the rails something to scroll, so a gold
thumb in a bevelled track with arrow buttons appeared down the right of the World, over the
frames. Pixel-perfect, in the pack's palette, and the first thing the owner pointed at.

An **immersion leak** — a careful imitation of a widget is still a widget. What tells you a list
continues is a row clipped at its edge: the content itself, not a control about the content.
Nothing is hidden except the widget.

**And removing it exposed the defect it had been explaining.** With no scrollbar, the Terminal's
bottom bevel was simply cut off — the rail wanted 725px of cards in 699px, and the scrollbar had
been the only thing accounting for the difference. A fixed `30vh` slice cannot know what the rail
can hold; cards that *can* give way now shrink to fit and grow into what is spare, with a floor
where they stop and the rail scrolls instead.

> **A widget that explains a layout problem is not a solution to it.** Take the widget away and
> the problem is still there — now with nothing to say so.

## An id identifies; a kind classifies (added 2026-08-25, from implementation)

Two Claude Code sign-ins on one machine were asked for, and building them found that Epoch had
been treating *a program* and *an account* as one thing everywhere — including in a gauge.

**Isolation was measured, not remembered.** `CLAUDE_CONFIG_DIR` and `CODEX_HOME` each separate a
sign-in completely, and neither appears in `--help`. Same discipline as ADR-0027's three facts:
somebody else's flag is a measurement.

**The rule, which is ADR-0023's applied one subsystem over:** `claude-code-2` is *who*;
`claude-code` is *what kind of program*. Both travel on the status, and everything that reasons
about the program keys on the kind. **No surface may take an id apart to work out which program
it is** — that would be the second place deciding, and two places deciding is how they come to
disagree the first time a third account exists.

**An allowance belongs to a sign-in, never to a program.** Measuring once per program and drawing
it on two rows is a *real reading of the wrong quantity* — the most convincing way a gauge can
lie, because something is genuinely being measured. Readings are keyed by account; a missing key
is *unasked*, a `null` is *asked and it could not say*, and neither is zero.

**Where nothing can be measured, the user is asked.** Claude Code says which email is signed in;
Codex cannot be asked at all. So the account carries the user's own label — *work*, *personal* —
because a title invented from an empty answer is exactly the gauge nobody can explain, and the
honest alternative costs one text field. Gemini CLI is refused a second account outright, in the
Engine's own words: an API key that cannot be asked about makes two rows indistinguishable.

**And Epoch still never holds the key.** Adding an account makes a folder and starts the
program's own login in its own window. `AgentAccount` has no credential field, so a stored
account is *structurally incapable* of holding a token — a guarantee the compiler keeps rather
than a review checklist.

**Known and not built, so it is not claimed:** the World runs **one turn at a time** by design,
and two single-slot fields depend on it — the open decision waiting on the user, and the Quest a
running agent turn files evidence against. Two accounts therefore do not give two characters
working at once; that is a separate feature with a real cost, and misfiled evidence is worse than
missing evidence (ADR-0025).

## A measurement is only honest about the box it was taken in (added 2026-08-25, from using it)

Five things reported by opening the product, maximising it and watching Task Manager. Three of
them are the same mistake at three altitudes, and it is worth naming as one.

**A percentage is about the box it resolves against.** A scrolling card was capped at `32vh` to
stop one list eating the rail — right about *what*, wrong about *which box*. Maximised, `32vh` is
438px inside a card the rail had given 761px, so the list drew its own edge across the panel and
left a dead band under it. The rail is the box; the cap is a percentage *of the rail* now, and a
card no longer grows past its own contents — spare height belongs at the bottom of the column,
where it is World. `vh` in a column that is not the viewport measures something else, and it will
disagree with the layout on exactly the machines you do not own.

**A guard that exists to skip expensive work makes everything below it accidental.** 11.21 moved
`settings` above the Launcher's startup guard and left `agents` below it, so a second account was
invisible in CREW LINKS until a full re-probe — and, worse, an account that *had* appeared
**disappeared** on the way back from a World, because the remount re-seeded from the boot
photograph. A vanishing row is not a cold instrument; it is a wrong one. Reading the Engine's kept
survey is the cheap half of that pair. Fixing one field and leaving its neighbour is how a rule
becomes a coincidence.

**And a program gives nothing back unless it is asked.** ComfyUI held **7.1 GB of system memory and
6.6 GB of video memory** after a finished render — measured before and after, both pools — and one
`POST /free` returned all of it. So the owner's guess that it *was not even using the card* was
wrong in an interesting direction: it uses both and releases neither. The ask now happens where a
picture is finished, under the single setting the text runtimes already answer to. `/free` replies
an empty 200, so the claim comes from reading `/system_stats` back — 11.21's rule, and the reason
the test **draws first**: its first version asserted on a precondition it had not established and
failed against a perfectly healthy server.

**A form is not a window.** The picture panel was in the right place and was the wrong object — a
translucent slab pinned between the conversation and the composer. It is a `Frame` now, like every
other window, so a World Pack re-skins it with the rest; and it lives *in* the log rather than under
it, because a thing a character handed you belongs inside the record of them handing it to you.

**And not everything reported is a defect.** A sign-in that "closes like Ctrl+C" and "will not let
me paste" is a browser callback completing the login and a fallback code nobody needed. Epoch opens
the door and never holds the key, so what it can fix about a confusing doorway is **describe it** —
and re-read the agents when the window regains focus, which is a real cause rather than a poll: the
user was elsewhere and has come back.

## A default is only safe where the measurement is a comparison (added 2026-08-25, ADR-0033 amendment)

The Studio Panel worked in every part except the one it exists for, and every fix came from
somebody filling it in and being unable to explain what they were looking at.

**A button that asks somebody else to do the thing it is named after does not work.** GENERATE
recorded the settings and handed the prompt to the character, so the picture would arrive as their
reply and land on the Quest as evidence. Right about evidence, wrong about what happens: measured,
a model handed the prompt with the panel already open re-opened the panel and said *"choose what
you want and press Generate."* GENERATE draws now, and the evidence half is **kept and moved** —
the Engine files the picture on the active Quest in the same `Produced` entry a capability's run
makes. Nothing is said afterwards, because saying it made the model draw the same picture a second
time. **Nothing asked of a model costs nothing and can misread nothing.**

**A control keyed to how far somebody has got will be wrong until they have finished.** For a Flux
model the family list showed twenty-eight names with `flux2` among them and no `flux` — measured,
`CLIPLoader` publishes no `flux` and `DualCLIPLoader` does — because the list followed the number
of encoders *picked so far* rather than the number the measured family *takes*. It only became
answerable after the step it existed to help with, and offered a near-miss until then.

**Marked versus chosen, and where the line actually falls.** ADR-0033 says the panel is the user's,
and 11.22 read that as *mark, never select*. The rule is about **files** — which checkpoint, which
LoRA, which encoder. An encoder family is not a file: it is a property of the model Epoch read out
of its own tensors, with one right answer, and leaving it blank moved the guessing onto the user
rather than out of the product. It arrives chosen; every other family stays exactly as selectable,
which is what keeps it a default rather than a decision.

**And the VAE was nearly given the same treatment.** The only VAE whose metadata named the family
looked like the obvious default — and it chose the *Z-Image* autoencoder for a Flux model, because
that file says `Flux.1-AE` while the actual Flux VAE says nothing. Epoch does not read filenames
(ADR-0024), and a property that only one candidate happens to carry is not a comparison:

> **A default is only safe where the measurement is a comparison.** One file answering a question
> the others were never asked is not evidence that it is the answer.

**Three ratios was a decision nobody made on purpose.** Offering only what the family was trained
at quietly decided that nobody wants a wallpaper. The sizes people name out loud are there now,
with the trained ones **marked** — asking SD 1.5 for 1024 gives two heads and asking Flux for 4K
gives an out-of-memory, both true, and neither Epoch's call to make for somebody who owns the card.

**A control whose ends are not written down is a control you have to guess at.** The LoRA strength
was an unstyled range input — the platform's blue slider inside a pixel-art window, an immersion
leak — and it was read backwards. Both ends are on it now.

**And ComfyUI holding 2 GB after being asked to let go is not a leak**: 954 MB freshly started
having never loaded anything, 8957 after a render, 2201 after the ask. What stays is the server and
torch's CUDA context, and the only way to zero is to stop the server.

## Measure the thing you can always measure (added 2026-08-25, from using it)

11.26 taught the panel to name the encoder family for Flux and SD 3. The owner's next model was a
Z-Image and the panel was back to twenty-eight names and no help — because the measurement being
used was the one Epoch can rarely take.

**Epoch reads a checkpoint's family for five families and an encoder for every file.** `Base` knows
SD 1.5, SD 2, SDXL, SD 3, Flux; everything newer is `Unknown`. An encoder is readable from the
width of its embedding table, always — and it is what **ComfyUI itself keys on**: `comfy/sd.py`
picks the text-encoder implementation from the detected model and reads `clip_type` only to tell a
Flux/Klein setup apart. Verified by drawing, not by reading the source: the same Z-Image graph drew
as `stable_diffusion` and as `qwen_image`, and failed as `flux2`.

**But after the stronger measurement, never before it.** Written the other way round, a Flux model
with one CLIP-L picked so far was told *"a CLIP-L on its own is how Stable Diffusion is loaded"*
and *"Epoch measured this model as Stable Diffusion."*

> **A partial choice is not evidence about the thing being chosen for.** It is evidence about how
> far somebody has got, and reading it as the first is how a real measurement gets overwritten by a
> guess about it.

**And a regression worth keeping the shape of.** 11.26 keyed the family list on how many encoders
the family *takes* rather than how many were picked — fixing a list that could not contain the
answer, and opening a worse hole: the panel offered `flux` from the double loader's vocabulary
while one encoder still built a single loader, so every configuration was refused with `'flux' not
in (list of length 28)`. The loader is chosen by the count of files; only the list had moved.

> **When two things must agree, derive both from the same measurement.** Moving one to a better
> source and leaving the other where it was produces a defect that lives only in the combination
> neither side is watching.

The fix is the one that keeps them derived from the same number: the panel waits until the measured
count is picked and **says what it is waiting for**. A dead control that explains itself beats a
refusal after the form was filled in perfectly — the cold-instrument rule, applied to a button.

## A reading that is correct, present and unusable (added 2026-08-25, from using it)

The Studio Panel drew, and the owner still could not fill it in for Flux. Every fact he needed was
on the screen and measured correctly, and none of it reached him.

**A label that read as the opposite of what it meant.** `SECOND ENCODER · Flux uses one` was about
that field and was read as *Flux uses one encoder* — so he picked one, found GENERATE grey, added
a second at random and got a server refusal. It says the **total** now.

**The sentence that would have saved him was the least legible thing on screen.** *"Flux is loaded
with two text encoders (a CLIP-L and a T5-XXL)"* was painted in the tone used for prose you read
*afterwards*. There is a real difference between a hint that **describes** a control and one you
must read **to use** it, and it had never been drawn.

**And two correct facts in two places still had to be joined by eye** — the guidance named the two
encoders, each row named what each file was, and the reader did the matching, once per field. A
family now declares what it `wants` in the same words a file is named with, so the rows mark
themselves by comparison. Nothing typed, nothing guessed.

> **Epoch measured it, Epoch said it, and the user still could not act on it.** That is the fifth
> face of the cold-instrument rule and the hardest to see, because nothing is missing, nothing is
> invented and nothing is wrong. A reading that is correct, present and unusable has not arrived.

**And an honest answer about the future beats a comfortable one.** *"If I download a new model
tomorrow, will it work?"* has four answers, not one, and they are written down in
`docs/Build/The Picture Chain.md` with the measurements behind them: a checkpoint always works; a
model in parts whose **encoder** Epoch recognises works even when Epoch has never heard of the
model — which is why Z-Image draws today; one with an unreadable encoder still opens, still draws,
and simply offers no suggestion; and a video model does not work at all, because `asset.rs`
delivers a PNG and nothing downstream can display a video.

Upscalers and ControlNets are named there as what they are: filed correctly, installed correctly,
**and consumed by nothing**. A shelf whose contents nothing can use is worth writing down before
somebody fills it.

## A machine that agrees with the mistake it was handed (added 2026-08-25, from using it)

The owner opened the picture panel on a Z-Image, picked the first encoder in the list because
nothing said otherwise, and Epoch told him he was right: *"A CLIP-L on its own is how Stable
Diffusion is loaded. Epoch measured this model as Stable Diffusion"* — and then **marked that same
file as the one this model needs.** Every render failed.

Both sentences came from a correct rule about a correct measurement. Together they were a circle:
he picked a file, Epoch inferred a family from the pick, and then used the inference to endorse
the pick.

> **A derivation from the user's own input must never be presented as evidence about the thing
> they were choosing for.** It can only ever tell them what they have already told it — and when
> it points back at what they just did, they have no way left to find out they were wrong.

The mark was the worse half. A guess presented as a guess is survivable; a guess that *points at
what you already chose* is agreement. So: the fallback opens by saying the family could not be
read, describes what **was picked**, invites a change, and **marks nothing** — marking a row is a
statement about the model, and only a family read from the model's own tensors may make one.

**And the real fix was to measure the model.** ComfyUI's own detection tells Lumina 2 from Z-Image
by the first dimension of `cap_embedder.1.weight` — 2304 against 3840 — which is a shape in a
header Epoch already reads. A rule read out of the program rather than remembered about it, and a
shape rather than a name (ADR-0024). Plain Lumina 2 stays unmeasured, because nobody has drawn
with one here and filing it by association is the same guess one step along.

> **When a fallback starts being load-bearing, the fix is upstream of it.** Softening its wording
> was necessary and would not have been enough: what the user needed was for the question not to
> reach the fallback at all.


## The last thing read is the question (added 2026-08-26, from using it)

Reported as *"los NPC perdieron la personalidad y ya no llaman tools ni skills"*. Both halves were
one defect and it was in the **shape of the turn**, not in the model, the tool surface or any
prompt.

Everything the Composer adds from step 5 on — the terminal note, the connections, the tools block,
the stand-aside instruction — sits at the end because a model weighs the end of a conversation
most, and each was moved there to win a real argument against the Chronicle. All three were right.
Together they took the final position away from the only text with a claim on it: an eleven
-character request followed by nine kilobytes of standing instruction is a turn whose last question
is *did you understand your instructions?*, and a model answers the question it was actually asked.

Measured on the recorded turn that showed it — `vault/last-turn.json`, replayed against the live
Ollama, one change at a time. Cutting the two MCP servers' 32 tools: still recited. Moving only the
tools block earlier: still recited. Moving **the user's words last**: `open_studio`, twice — and
*"Hola"* answered as Mage, in Spanish, in 7.3 s.

> **The blocks have to come after the Chronicle, not after the person.** They are still the last
> *instruction* read. They are no longer the last *sentence*.

And the three defects that put them there stay fixed, re-measured from the new position: a
Chronicle full of *"I cannot read files"* still calls `read_file`, and *"tell Paladin to add a
line"* still defers to Paladin.

## A gauge that identifies nobody (added 2026-08-26, from using it)

Two characters on two Claude Code accounts drew identical cards: `haiku`, `haiku`. The allowance
meters underneath were **already keyed by account and already correct** — so the difference was on
screen the whole time, as two numbers with nothing naming whose they were. The only way to find
out was to ask the character, and a character's answer about its own sign-in is a self-report
rather than a measurement. It happened to be right, because an agent can read its own
configuration; a local model would have invented one.

A sixth face of the cold-instrument rule, and the quietest: **a correct reading of the right
quantity, with nothing saying what it is a reading of.** The fix is a qualifier, and it appears
only when the machine has two accounts of one program — naming the only sign-in there is would be
an instrument that never moves.

## A derived control is not chosen for by setting its value (added 2026-08-26, from pressing it)

`USE THAT` offers back the combination a model was actually drawn with. Pressed on a Z-Image it
filled in the encoder and the VAE and left `stable_diffusion` on screen — **the one value the
recipe exists to correct.**

The family field is derived from Epoch's measured suggestion *until somebody chooses*, and setting
the state is not choosing. So the suggestion kept winning against the evidence that was meant to
replace it.

> **Anything that fills a derived control in must also say the choice was made.** A value and a
> `touched` flag are one fact stored twice, and the half that is forgotten is always the one
> nobody is watching — here, the field that looked right until it was read.

## Evidence beats a rule that will never be complete (added 2026-08-26, ADR-0032's amendment, built)

Epoch reads a checkpoint's family for six families and never will for all of them. But a graph
that **ran** is a fact about that exact model, filed by its hash, and it costs nothing because it
already happened. Two recipes are two things that worked, not a contradiction — and only a refusal
*about the configuration* is kept, never an out-of-memory, which is the card that day.

Where the hash is measured was a trade and it was measured: 1.8 GB/s here, so 3.4 s for a 6.2 GB
model. **Never on a read path** — a panel that hashed on open would cost seconds every time
somebody looked at it — and always after a picture, where the render already cost tens of seconds.

## A catalogue is not a reading (added 2026-08-26, ADR-0030's second amendment, built)

Six Style names shipped and five sat dark on every install. Defended once as the cold-instrument
rule, and the defence does not hold: a dark **REACTOR LOAD** is a real quantity not yet wired; a
dark **Anime** was never a quantity at all. **The rule protects readings, not catalogues.**

So a Style exists because a workflow serves it and stops existing when the last one is taken away.
Naming one on a workflow row is what creates it — Epoch never reads the name off the file, because
a graph called *SNES Pixel Art* is probably pixel art and *probably* is not a thing to act on when
the consequence is deciding what somebody's `pixel art` means.

The same session's other half is the mirror image: an export **says** which files its declared
licence was never written about — artwork the user imported — and does not refuse over them. The
hard rule governs what Epoch *distributes*; what somebody hands a friend from their own vault is
theirs, and Epoch cannot read what a picture is or who made it. **Refusing on a question you
cannot measure is the invented gauge wearing a veto.**

## A test that prepares its own input proves the wiring and nothing else (added 2026-08-27, from using it)

Two defects in the picture chain, reported as one screenshot each, and neither was reachable from
the 1000-odd tests that pass.

**Hand-numbered nodes collide the day a fourth thing is added.** The fixed graph is `1`–`10`;
LoRAs counted from `11`, the upscale pair was written as `11` and `12`, and steerings counted from
`13`. One LoRA and an upscale both claim `11`. The map is keyed by id, so the later push simply
won — the `LoraLoader` stopped existing, both `CLIPTextEncode`s stayed wired to slot `1` of what
had become a one-output `UpscaleModelLoader`, and ComfyUI answered `list index out of range` about
an input called `negative`. Every piece was individually correct and tested. One counter cannot
collide with itself; three blocks of numbers can, and the arithmetic that decides whether they do
is exactly what nobody re-checks. The regression test asserts **no numbers** — it follows every
wire to the class on the other end, which is the property the ids existed to keep.

**And the reference was handed over raw.** A canny ControlNet reads *edges*; given a photograph it
draws noise. Three stacked steerings was the reported symptom and one does it. The live test that
had passed a week earlier used black shapes on white — already edge maps:

> **A test that happens to supply prepared input proves the wiring and says nothing about what a
> person will actually drop in.** It is 11.18's rule (a harness declaring its own tools measures a
> path Epoch does not take) arriving from the data side instead of the tool side.

Each row now asks what the ControlNet should read and `GENERATE` waits for the answer. **Nothing
is preselected, because there is nothing to measure**: which preparation a ControlNet wants is a
property of the ControlNet, the only thing naming it is the file name — which ADR-0024 forbids
reading — and an edge map and a photograph are both an image. Reading silence as *already
prepared* is what drew the noise, and it is the same inversion as reading it as *no*.

**What is offered is a named set filtered by what the server has, never a category and never a
shape.** `Canny` sits in `image/filters` beside `ImageBlur` and `Morphology`; no category on this
machine contains the word *preprocessor*; and *takes an IMAGE, answers an IMAGE* is sixty classes
wide and mostly hosted-API photo editors. One entry, because one is what was measured — and *it is
already a control map* is always offered, so a server with no preparation still steers. Every
other value on the inserted node is the server's own declared default: asking somebody for two
canny thresholds they have no way to choose between is how a panel stops being a panel.

## A rule is not its implementation (added 2026-08-27, ADR-0024 §2b)

FILES lagged with a heavy picture in it, and the first instinct was that ADR-0024 was in the way:
it says images cross as base64, and base64 is what was slow. Both halves of that were wrong.

**The rule was never *base64*.** It is that the presentation layer has no filesystem access and
the Engine names the file. Base64 was how that was achieved on the day it was written, and reading
the mechanism as the decision is what made a solvable performance problem look like an
architectural one. Pictures now reach the window over `epoch://picture/<name>` — the page hands
over a **name**, a handler in the Engine decides what it resolves to, no path and no `fs`
capability go anywhere near the webview. Every guarantee is intact and the transport changed.

**And base64 was a third of the cost, measured, not the wall.** On a 50 MB PNG: 70 ms to undo it
against 148 ms to decode the picture — and the picture decodes whatever the transport is. What the
scheme actually buys is that **JavaScript never holds the bytes**: 3401 ms → 1082 ms for one
182 MB render, and **0 ms** the second time, because the browser caches it under a name that is
the hash of its own contents. Fifteen thumbnails in 1.21 s.

> **Before blaming a constraint, check whether you are looking at the constraint or at how it was
> implemented once.** The load-bearing sentence is usually shorter than the paragraph around it.

**The check that means anything is the one the thing actually answers to.** The first measurement
used `fetch`, got `Failed to fetch`, and would have been read as *the scheme does not work* — but
`fetch` is governed by `connect-src`, which was deliberately not widened, and a picture is drawn
by `img-src`. Testing the wrong governor produces a confident wrong answer, which is worse than no
answer. Same family as 11.18's harness that declared its own tools.

**Two guards, and neither leans on the other.** `../../../vault/secrets.dat` is refused because
`convertFileSrc` escapes every separator into one name that is not in the folder, *and* because a
URL written by hand with real separators is cut to its last component. A defence that only works
through the front door is a defence with a back door.

**Giving the frontend the filesystem was considered and is worse for this**, which is the part
worth keeping: `readFile` still hands the bytes to JavaScript, so the page still holds a copy and
still decodes on its own thread — it saves the 33% and nothing else, and costs the guarantee. The
narrower door beat the wider one on the metric the wider one was proposed for.

## One thing draws, and it is the panel (added 2026-08-28, ADR-0030's fourth amendment)

Two capabilities answered one intent, so a model could pick the one that skipped the person — and
it did. `draw_image` and `edit_image` are deleted. A character asked for a picture opens the
Studio Panel and the **user** makes it.

**This is what ADR-0033 already said**, arriving at last: the panel is offered *before* anything
is drawn. Keeping the older tool beside it was defended as *"hazme una imagen de Chrono must keep
working, or this is an application with a render button rather than a World"* — right about what
must keep working, wrong about what has to exist for it to. Measured in the window: the sentence
works, and it opens the panel in 11.4 s; GENERATE draws in 13.0 s.

**Every guarantee the old capability was carefully worded to hold is now held by construction.**
`open_studio` takes no arguments, so a character cannot name a workflow, invent a Style or guess
a description — the three failures ADR-0030's amendments were written to fix, each of them a
sentence in a descriptor that a model could read past. A tool with nothing to fill in has nothing
to fill in wrongly.

> **Two tools for one intent is a decision the model makes and the user lives with.** The wording
> of a descriptor is a request; the shape of a tool surface is a rule.

**And it closed a defect four diagnoses had failed to explain.** A turn that called `draw_image`
never ended — 901.4 s three times, which turned out to be the driver's own patience rather than a
latency — while the picture always drew and always landed correctly. Each hypothesis was
disproved by the next measurement. With nothing left to call, the turn ends. Worth naming because
the fix was not found by understanding the bug: **a defect that survives four correct measurements
is often a question about why the code path exists, not about what it does.**

**Codex is untouched**, and that is the rule rather than an exception. It draws with its own
`image_gen`; Epoch witnesses the result and never authorised the tool. Epoch does not govern an
agent's own tools.

**What survived the deletion, and why it is not dead code.** `Easel`, `Asked`, `Shape` and
`Detail` have exactly one consumer left: the Connections deck's chain test, which walks Style →
workflow → compile → server → kept file and needs to draw one picture through the real machinery.
The halves that parsed a *model's prose* into a size and an effort went with the capability —
nothing turns a model's words into a picture any more, which is the whole change. A test that
measured whether every local runtime could reach the brush was deleted rather than left
`#[ignore]`d: **a test nobody can run is a claim nobody can check.**

## A second medium changes three nodes, not the graph (added 2026-08-28, Phase 12, ADR-0034's amendment)

Video works: the Studio Panel's VIDEO tab, a Job that outlives the button, `WAITING` on the crew
card, and a video playing in the Chronicle. Measured in the window rather than reasoned about —
GENERATE, the card reads `WAITING` within the second, a video plays **40.3 s** later.

**The picture graph and the video graph are one function**, because they differ in three nodes: a
latent that takes a `length`, a conditioning that carries the frame rate, and a muxer instead of a
`SaveImage`. Writing a second `compose` would have duplicated the loader selection, the LoRA chain
and the ControlNet chain — and *when two things must agree, derive both from the same
measurement*. A test asserts the picture graph is unchanged, by following wires to the class on
the other end rather than by naming ids.

**`make_video` was in the roadmap as a capability and turned out to be a tab.** One thing draws
and it is the panel, so a capability that made a video would be the mistake ADR-0030's fourth
amendment had just deleted, arriving in a second medium. The Job machinery took a producer without
changing: `Outcome::begun` is still how a capability would start work; the panel calls
`Jobs::begin` directly, because a person pressing a button is not a turn.

> **A subsystem built ahead of its consumer is only proved when one arrives.** `jobs.rs` had six
> tests and no producer for a day. Everything in it was right, and what the producer found was
> everywhere *else*: a CSP directive, a sniffer, a loading shape, and two sentences that read as a
> stutter.

**Three things the product taught that reading could not:**

**A checkpoint may not carry its text encoder.** `ltxv-2b-0.9.6-distilled` loads through
`CheckpointLoaderSimple` like any checkpoint and answers `CLIP = None` — ComfyUI says so in as
many words. So `Loading` has a third shape, and it is **derived from what the person picked**: a
checkpoint with encoders chosen beside it is `Encoded`, the same file with none is `Checkpoint`.
Epoch never reads a file and decides it is missing something (ADR-0033).

**`media-src` is not `img-src`.** It was `'none'`, so a `<video>` was blocked by a directive
nobody had thought about while widening the one next to it. The same shape as testing `epoch://`
with `fetch` and reading `Failed to fetch` as *the scheme does not work*: **check the governor the
thing actually answers to.**

**And what a capability *produced* is a wider list than what a person may *import*.** `MadeFormat`
is the first; `ImageFormat` stays the second. Adding `Mp4` to `ImageFormat` would have been one
line and would have let a video become a World's key art and be handed to a model that can see —
a nonsense that arrives as a confusing failure three subsystems away. ADR-0024's guarantee is
about the *import* door, and widening it to serve an *export* need is how a guarantee quietly
stops being one.

**One family, because one has been measured.** `LTXV`, drawn on this card at 49 frames in 10.1 s
before a line of the implementation existed. Wan and Hunyuan have nodes on this server and no
model, so their recipes would be written from memory. `PREPARATIONS` has said this since 2026-08-27
and it is the same sentence: **a named set grows the day somebody measures one, not the day
somebody remembers one.**

**Refused where it would have been silent, on the thread where somebody is standing.** The graph
is composed before the job starts, so a missing node or an unknown family answers the button press.
A refusal arriving four minutes later as a message from a character is exactly what the deleted
`draw_image` did with its asynchronous `refusal()`.

**And the character waits rather than works**, which was the owner's correction to ADR-0034 §5 and
is now the code: `Effort::Waiting` is its own state, and the card reads `WAITING`. A World that
showed somebody working while a machine worked for them would be claiming something it cannot see.

## A setting that decides two things decides the wrong one (added 2026-08-28, from a report)

The owner pointed at the KEEP tooltip and said it was false. It was not: it described the code
exactly, and the code was wrong. **Run several crew members at once** governed two separate
questions and its default answered only the first:

| the question | what off meant | what it should mean |
|---|---|---|
| may **one** model stay loaded after it answers? | no | yes — it is what the card had a second ago |
| may **several** be loaded at once? | no | no |

The memory argument behind the default was real, measured, and about *several* — one 14B holding
4 GB with the crew idle, three runtimes each keeping a copy on a card that also draws. **One is
not several.** The World runs one turn at a time, so holding the last speaker costs one model, and
the cost of not holding it was measured at about 19 s on every single message.

> **A setting whose name names one thing must not decide two.** The half that is not in the name
> is the half nobody re-reads, and its default will be inherited from the half that is.

The fix keeps the name and moves the memory guarantee to where it is actually about several: a
*different* character speaking releases the previous one's brain first, so at most one is resident
with the setting off. Everything that already released still does — a render, a Job in flight, and
the user turning KEEP off — which is exactly the list the owner gave when he said what he expected.

**Two things this cost that are worth naming.** The tooltip's second half, *"following the
machine's setting"*, was true and became false with the fix — a sentence about *why* a value is
what it is has to move when the reason does. And the release is aimed by the provider and model
that were **recorded when the turn ran**, never the ones that character has now: a brain
reassigned in between would release the wrong model and leave the real one on the card.

**And the report was right about the product while being wrong about the sentence.** The tooltip
was accurate; what it accurately described was a decision nobody would have chosen if it had been
written on one line. That is worth more than a bug: a gauge that correctly reports a bad default
is how a bad default survives.

## The studio lives exactly as long as a picture takes (added 2026-08-28, from using it)

Four defects in the picture chain, three reported by the owner and the fourth found measuring
them. They share nothing except that each was a place where a cost nobody could see was being
paid by somebody who had not asked for it.

**A condition borrowed from another subsystem is a condition nobody re-reads when that subsystem
changes.** `Bench::release` — ComfyUI's *free your memory* — was gated on
`Settings::keep_loaded()`, reasoning that somebody who wants their models resident wants the one
that draws resident too. That reads a **text-model** setting to answer a question about a
**picture server**, and the day the setting stopped meaning *hold nothing*, the answer silently
inverted: 20.8 GB of Python in Task Manager after a finished render. The gate was never wrong
about ComfyUI; it was never *about* ComfyUI.

**Opening a form must not cost twenty gigabytes.** The panel started the studio on open, on the
reasoning that opening it is asking to draw and the thirty-second wait is better spent than
watched. Using it says otherwise — a process and a terminal window arrive for somebody who may
look and close it again. GENERATE starts it, waits, draws, and lets it go once the picture is in
the conversation.

> **Reversed the next day, 2026-08-29, by the owner — and the number in the heading is the
> reason it was wrong.** Twenty gigabytes is what ComfyUI holds *after a render*; freshly
> started and holding nothing it is **954 MB**, which was measured in this very section and
> read past. So the cost of opening the form was thirty seconds and under a gigabyte, and what
> it bought was the only list a shelf cannot stand in for.
>
> What the removal actually cost was the panel. A model that arrives in parts needs a family,
> that list is a *node's* vocabulary, and GENERATE waits for it — so the panel opened with a
> dead button and a sentence explaining that pressing it once would fix it. **A form that
> cannot be completed until you press the button that needs it completed is not a form.**
>
> **The cost was measured on the wrong side of the thing it was about.** Both halves were true
> — 20.8 GB is real, and so is a terminal window nobody asked for — and neither was a
> measurement of *opening the panel*. A number carried in from the paragraph above it is not a
> measurement of what the paragraph below is deciding.
>
> The release half is untouched and is what made the reversal cheap: closing the panel still
> lets the studio go, and everything below about reading the shelves still stands, because the
> shelves are what fills the panel while the machine is starting.

Nothing was lost, and the reason is worth keeping: **the server was never the only place those
lists were known.** They are files, and `search_paths` already answers *which folders ComfyUI
looks in* — so reading the names out of them gives the same list from the same files, seconds
earlier and for nothing. Listing, never adopting (ADR-0032). What genuinely needs a running
server is a **node's vocabulary** — which encoder families a loader accepts — and the panel says
so rather than showing an empty dropdown that reads as *you have none*.

> **A fallback that is empty by accident is one that stops being empty the day somebody drops a
> file in the wrong folder.** The family list was nearly routed through a shelf that happens to
> hold nothing loadable. It has its own path, and the emptiness means *nobody was asked*.

**And a closed port is not free.** One connection to `127.0.0.1:8188` with nothing listening
takes **21 seconds** on this machine — not the instant refusal loopback is supposed to give — and
a request's own timeout does not cover it, because it is the connect syscall. Nine such calls
meant a stopped studio made the panel take **190 s to open**. `checkpoints_at` had carried a
400 ms `timeout_connect` since it was written; three sibling probes never got one, and it was
invisible for as long as the panel started the server first.

> **A latency that only appears in a state you never enter is a latency you will meet the day you
> change when that state happens.** Nothing here was slower than it had been; a path that was
> rare became the ordinary one.

**A mark on everything marks nothing.** `KEEP` drew itself gold whenever it was holding, which
was worth drawing while holding was the exception. It became the default, so every button in the
World went gold — and the owner noticed the button had changed shape before he noticed anything
else. Gold now means *pinned on purpose*, dim means *let go on purpose*, and the plain button is
what everybody nobody has touched still gets. The lamp keeps saying what is actually on the card,
which is the fact that moves.

**Measured from a cold machine, which is the state that had never been tested**: the panel opens
instantly with what is on the shelves, GENERATE puts a picture in the conversation 26 s later
including starting the server, and afterwards ComfyUI is stopped, its memory is back, and the
character's model has returned to the card.

## Three answers, and the third is the one that keeps somebody's file (added 2026-08-28, from using it)

The Studio Panel listed every model alphabetically, so the first thing offered for a picture was
a video checkpoint — which answers `clip input is invalid: None` minutes after the form was
filled in correctly. Splitting the list by medium was the obvious fix and the interesting part
is what it must not do.

**Measured, never named.** `patchify_proj.weight` beside `adaln_single` is LTXV — ComfyUI's own
key, read from the owner's file. `Base` grows an arm the same way `ZImage` did, and for the same
reason: a family Epoch cannot read is a panel that cannot help.

**But `Base::moves` returns `Option<bool>`, not `bool`.** A family Epoch measured belongs to one
medium; a family it could not read belongs to neither, and `None` says so — such a model is
listed under **both** tabs.

> **Hiding somebody's file because Epoch failed to measure it is the worst outcome a filter can
> produce.** There is nothing they can do about it and nothing telling them why. A boolean would
> have made *unreadable* mean *not this one*, which is the same inversion as reading silence as
> *no*, wearing a filter instead of a gate.

On this machine that case is real and not hypothetical: `minimax_h3` is genuinely a video model
and genuinely unreadable, and it appears in both lists rather than vanishing from the one it
belongs to.

**Two more the window found, and neither was reachable from a test:**

**A gate with nothing behind it must stay open — again, one layer up.** With ComfyUI stopped the
VIDEO tab was dark, because the families are node classes and there was nothing to ask. That
turned *not asked* into *no*, on a machine with a video model sitting on it. Now, with nothing
serving, the families come from the models on the shelves — still a measurement, since Epoch read
`LTXV` out of that file's own tensors — and `compose` refuses by name at the door if this ComfyUI
lacks the nodes. **The weaker measurement is the honest fallback; the absent one is not.**

**And one control may not mean two things.** The encoder family is a *node's* vocabulary, so no
shelf can stand in for it, and GENERATE waits for a family — which with nothing running is a
button that can never be pressed. The tempting fix is to let GENERATE start the machine on the
first press and draw on the second. That is a control whose meaning depends on hidden state, and
this codebase has already deleted one of those (`Manual`, ADR-0027). The panel offers
`START THE DRAWING MACHINE` in exactly the place the dead end appears, and GENERATE keeps meaning
one thing.

## The medium changed and the shape did not (added 2026-08-29, Phase 12 closed)

Video, sound and 3D all reached the Chronicle, and the thing worth writing down is that **none of
them needed a new abstraction.** A `Beyond` on the `Ask`, a `Keeping` naming which nodes end the
graph, a `MadeFormat` sniffed from the bytes, one `epoch://` scheme, one Chronicle row. Had a
`make_video` capability been written instead, there would now be four of everything — which is
ADR-0030's fourth amendment paying for itself in a medium it was not written about.

The two cold tabs of ADR-0033 were **lit by their own stated conditions being met**, not waived.
That is what a cold instrument is for.

**A tool result key is part of a recipe, not a constant.** `SaveAudio` reports under `audio`,
`SaveGLB` under `3d`. The picture chain read `images` and only `images`, so the first sound
generated perfectly and arrived as nothing.

**And a widget that imitates the World is still a widget.** The platform's `<audio>` element in a
pixel-art window is the same immersion leak as a drawn scrollbar — worse, because it also drew at
`width: 0`, being a percentage of a content-sized button with no intrinsic size. The transport is
drawn in the World's palette now.

### Comparing the subject is not comparing the render

A 3D `SURFACE` control offers *smooth*, *fine* and *blocky*. Fine was nearly not shipped: two
turntables at octree 256 and 512 were compared and called identical, on the strength of the
**animal** — same face, same ears, same horns. The owner looked at the **ground**. At 256 the disc
under the cow has concentric steps and the grass tufts are jagged; at 512 it is a clean ellipse.

> **A voxel grid — or any sampling artefact — shows itself on flat surfaces and thin detail, which
> is exactly where nobody is looking.** The thing being generated is where the eye goes and the
> last place the defect appears.

Six minutes against one, measured end to end through the panel (359.2 s against 61.8 s), named on
the control, and the person decides.

### A best-effort promise made with `?` is not one

Building `fine` found the defect that justified it. A 2 805 800-face mesh made wgpu answer

```text
Buffer binding 2 range 134678400 exceeds `max_*_buffer_binding_size` limit 134217728
```

by **panicking**. `preview` promised in its own comment that *every failure here is `None` and the
caller carries on* — and `?` covers the errors that are returned and none of the ones that are
raised. The model sat finished in the vault while the crew card read `WAITING` for twenty minutes.

> **A job's optional last step must not be able to take the job down** — not because previews are
> unimportant, but because the thing that was actually asked for had already succeeded when it
> failed.

Two fixes, because either alone is half of it: the panic is caught, so the comment is true again;
and the mesh is registered in **parts**, because the limit is 48 bytes a face and this cow was over
by a third of a percent. Splitting draws the whole surface — dropping triangles to fit would be a
preview with holes in it, which is a picture of a different model.

## Two spellings of one fact agree by luck (added 2026-08-29, from using it)

Reported as *"por que el boton de audio esta gris?"* — with both audio checkpoints sitting on the
shelf. A model's family travels as an **id** (`stableaudio`) and the graph table is keyed by a
**name** (`Stable Audio`), and the panel compared them with `eq_ignore_ascii_case`. That is true
for `ltxv` and `hunyuan3d`, whose id is the name lowercased, and false for every family whose
name carries a space or a hyphen.

So VIDEO and 3D worked **by luck** and AUDIO was dark. Both sides come from `Base` now —
`id -> Base -> that Base's own name` — which is one measurement read twice rather than two
spellings that happen to match.

> **The half of a comparison nobody is watching is the half where the two spellings differ.**
> When one fact is written down twice, derive both from the same measurement or the agreement is
> a coincidence with a shelf life.

**And the sentence on the dark tab was wrong in the other direction.** *"This ComfyUI has no
audio nodes"* is a reading of a machine nobody asked: with nothing serving, Epoch reads the
families off the shelves instead — which is still what fills the panel's first paint now that
opening it starts the machine again, because the machine takes thirty seconds to answer. A dark tab now
says which of the two absences it is, and never names a server it did not reach.

## A combo declares its answer as a list (added 2026-08-29, from building it)

ACE-Step 1.5's encoder takes fourteen values. Epoch knows three of them — the tags, the lyrics
and how long the song is — and fills the rest from what the node declares, which is the same
discipline a preparation's thresholds already followed: *asking somebody for numbers they have no
way to choose between is how a panel stops being a panel.*

It refused anyway. `timesignature` and `keyscale` are `IO.Combo.Input(options=[…])` with **no
`default`**, so reading `default` alone omitted them and ComfyUI answered
`required_input_missing: timesignature` after the form had been filled in perfectly.

The first option **is** the declared answer — it is what ComfyUI's own editor puts in the box.
So this stays read rather than invented, and a widget that declares neither still contributes
nothing, so the graph refuses **by name** rather than by guess.

> **A default is not always spelled `default`.** A schema can state a value by shape as well as
> by key, and reading only the key is a measurement that stops one field short.

## A song is told two things (added 2026-08-29, from being asked for one)

Sound worked for a day with the lyrics hard-wired to an empty string, and a comment in the
composer saying so: the panel had one box, and splitting one prompt into two would be Epoch
writing words nobody typed. That was right about *where* the fix belonged and it left the product
unable to do the thing the family exists for.

`LYRICS` is that box, and it is offered by a family whose encoder takes one and by no other —
`lyrical` is a field on the family table, because which node takes lyrics is a property of the
**encoder**, not of the medium. Stable Audio is told a description only, so its tab has none: a
control that reaches nothing is worse than a control that is absent.

> **A comment explaining why something cannot be done is a feature request with a due date.**
> It was correct on the day it was written; it stopped being a reason the moment somebody wanted
> the thing.

Empty lyrics stays perfectly valid. That is an instrumental.

## A document answers one question, so ask it there (added 2026-08-29, from one question)

*"¿todo esto está en roadmap y documentado?"* — asked after a session that had updated the
roadmap, three ADRs, the constitution and the handover. Checked rather than answered: the one
thing missing was the thing that had just been **recommended as next.**

**Concurrency had been the agreed next piece of engineering for four days and existed only in
`docs/Build/Handoff.md`.** `ROADMAP.md` went from Phase 12 straight to the Universe, so anybody
reading the file that lists phases would not have known it was coming. It is Phase 12½ now.

This is the sixth pillar's rule (*every document answers exactly ONE architectural question*)
failing from the other direction. The pillars stop two documents from answering the *same*
question; nothing was stopping a question from being answered in the **wrong** one. The handover
is where a session hands over. The roadmap is where phases live. A phase in the first and not the
second is invisible to every reader who was not in that session — which is all of them.

> **Being written down somewhere is not being documented.** A fact in the wrong document is
> reachable only by somebody who already knows it exists.

**Paid again on 2026-09-08, in public.** The repository went to GitHub with the *internal*
README — the one that explains the six pillars to somebody who already works here. The 367-line
one written for a stranger sat in a folder **outside the repository**, described only in a
session handover that is itself gitignored. `LICENSE`, `NOTICE` and `CONTRIBUTING.md` came from
that same folder, so it was found and read; the README was simply not on the list somebody made.
The file that exists to be read *first* was the one nobody could reach.

Two things this shares with the rules it sits beside, and the family is worth seeing:

- **A record of a hole must be closed in the same commit that closes it** (2026-08-23). The same
  session had to fix that too: two phases closed days earlier were still listed as open, copied
  forward from the previous handover instead of re-measured. *A list inherited is a list nobody
  checked.*
- **A gauge with nothing behind it must read empty.** A roadmap that skips a phase is the
  instrument version of the same failure — not a wrong reading, an **absent** one, on the only
  panel somebody would think to look at.

**And the answer to a question about documentation is a measurement, never a recollection.** The
honest reply here was *"no, and here is the hole"* after grepping for it — arrived at exactly the
way every defect in this file was.

### The measurement was of the wrong thing (added 2026-08-30, by the next session)

**Everything above is true and the phase it was written about had already been built.** Phase 12½
shipped on 2026-08-26 as `310eae4` — `running` and `waiting` keyed by character, a per-character
`Working` guard, a caret that names whoever is speaking — measured in the window with two
characters answering at once. Four days later it was written into `ROADMAP.md` as forty-six lines
of work to come, by the same session, in the same pass that correctly re-measured two *other*
entries and closed them.

So the grep that produced *"no, and here is the hole"* asked **is this phase in the roadmap?** and
never asked **is this phase still open?** — and only the second question had an answer in the code.

> **A measurement answers the question you asked, and it will not tell you that you asked the
> wrong one.** Grepping a document proves what the document says. Only the code says what is true.
> When the two disagree, the document is the one that is wrong, and the more carefully it is
> written the longer that takes to notice.

**The entry that escaped was the one being recommended, and that is not a coincidence.** A
recommendation reads as fresh — it is the sentence a session composes rather than copies, so it
feels re-measured even when its content was inherited whole. The three entries that got checked
were the ones being reported *on*; the one that got promoted to a phase was the one being reported
*forward*.

**Hence the operational form, which is narrower than "re-measure the list":**

> **Re-measure the entry you are about to act on before the ones you are only mentioning.** The
> cost of a stale entry is paid by whoever trusts it most, and that is always the person doing the
> next piece of work — which, for a handover, is the reader rather than the writer.

It is also the third time *a record of a hole must be closed in the same commit that closes it*
has been paid for, and the first time the payment was made by the document written to enforce it.
The correction is left visible in all four files rather than tidied: `ROADMAP.md`'s Phase 12½
keeps its reasoning under a *done* heading, and `docs/Build/Handoff.md` and `Claudetransfer8.md`
keep the sentence that was wrong, struck through. **A handover that quietly repairs its own record
teaches nothing.**


## A gate that passes with nothing to check has not passed (added 2026-08-31, from pressing RETRY)

The machine was rebooted, the benchmark was resumed, and Epoch answered:

> *"The control reproduces: 40.1 tok/s. This machine is itself again, and a new search can start
> from the beginning."*

It had compared nothing. The clean reference was **46.0** — 40.1 is 12.8% short — and
`may_begin(rate, None)` returns `Ok`, because a machine with nothing measured must be allowed to
start its first search. So the check passed *by default*, cleared the paused session, and offered
to run twenty minutes of measurement on a machine it had just declared healthy without looking.

**One fact written in two places, and the gate was reading the empty one.** A pause records
`clean_reference`; the gate reads the `references` list; and only `optimize` ever writes to that
list — a search that stopped before reaching it leaves the number on the record in front of the
gate and nothing in the place the gate looks. The banner had been showing 46.0 the whole time.

**And the rule it looked like it was following is a different rule.** *A gate with nothing behind
it must stay open* is about not withholding a capability: reading silence as **no** fails in the
direction that looks responsible. This is the mirror image, and it is worse, because it fails in
the direction that looks like success:

> **A permission may default to yes; a verification may not.** One that passes because it had
> nothing to check against has not passed, and reporting it as a pass is the invented gauge
> wearing a verdict — at the exact moment somebody is deciding whether to trust the machine.

*Nothing was compared* is now its own answer, the reading is still stated because it is the only
real thing in the sentence, and **the paused session stays** — clearing a record on a verdict
nobody reached destroys the evidence and the reason it was kept.

**The stand-in tolerance is the floor and says so.** The spread behind a remembered rate is gone,
so `Tolerance::around` allows 3% — the narrowest band this codebase ever uses. Narrow is the safe
direction here: it can only refuse more than the true band would, never less. What it must **not**
be is the paused session's own tolerance, derived from the controls that were already collapsing
(median 28 against a clean 46) — a test asserts that one would have approved 40.1, which is
exactly why judging a recovery by the spread of the illness passes anything.


**And the number that started it was not a fault.** With the gate fixed, LM Studio closed and the
model re-loaded, the same control read **46.7 tok/s over 3 runs** — at the historical figure, not
13% under it. The 40.1 was one median taken minutes after a reboot, and *a first load is not a
latency* has a section in this file already.

So the false verdict was the whole defect, and it was **false in both directions at once**: it
declared a machine healthy without looking, and the evidence available to contradict it was itself
untrustworthy. Had the gate simply been pointed at the 46.0, it would have refused a machine that
was fine — which is why the fix is *provenance*, not *a better lookup*.

**Hence a reference is now an entity rather than a number.** `PerformanceReference` carries the
artefact, the build, the GPU backend, the card, the driver, the OS, the context, the cache, the
flash-attention state, the speculation, the offload, the workload and its version — and
`is_comparable_to` answers **Comparable · Differs · Incomplete**, where the third is neither of the
first two. Silence about a field is not evidence that the field matched.

> **A performance figure without provenance is an anecdote.** It may be true and it may be about
> something else, and there is nothing in the number that says which.

The two llama.cpp builds on this machine are the argument in one line: same GGUF, same card, 4.5×
apart, because one is CUDA and one is Vulkan. Anything comparing across them reports a *package*
as a degraded machine.

**A fourth state, because two absences are not one.** `RECOVERY_REQUIRED` means *I know what
healthy looked like and I am not there*; `REFERENCE_REQUIRED` means *I have nothing comparable
enough to tell*. Reporting the second as the first sends somebody hunting a fault nobody has shown
exists — the same invention as declaring the machine clean, pointing the other way. Calibration is
permitted from there and comparative profiles are not, and CALIBRATE is a **separate press**: a
check that quietly minted the reference it had just failed to find would make the next check pass
by construction.

**The legacy figure is kept, labelled and demoted.** `46.0` with no provenance is still real
information about this machine — somebody measured it — and it may not decide whether a later
number is a regression. Deleting it throws away evidence; promoting it lets a memory with a
decimal point gate hours of work.

**And a benchmark is not a turn.** `KEEP` holds the last speaker resident so the next message is
instant, which is right for a conversation and wrong for a measurement: nobody is talking to the
model, the person is reading numbers, and the card it holds is the state the *next* control starts
from. The rule and its wording already existed one function away, on `TIME IT`, and had never been
applied to the search that needs it most.


## The instrument is part of the measurement (added 2026-09-01, from measuring twice)

Two calibrations of one experiment, back to back, nothing changed between them: **39.68** and
**41.90** tok/s. Three runs each, and the **run sets do not overlap** — A tops out at 40.50, B
starts at 41.44 — while placement, shared GPU memory, VRAM, RAM, prompt throughput and TTFT stay
put. An hour earlier a clean-state check of the same nominal configuration read **46.7**.

**No cause is claimed and that is the point.** Three cells, n = 1 each. What the pair proves is
not *why* the number moved but that the instrument had variables nobody had named — and an
unnamed variable in a benchmark is one that will be attributed to whatever you happened to change.

**The refused fix is the instructive one.** The obvious repair was `instrumented: bool`. The owner
threw it out: two different instruments are both `instrumented = true`, and the hidden variable
comes straight back wearing a name that looks like it was handled. So the fingerprint carries a
**protocol id** that spells out every knob — lifecycle, warmup, run count, sampler intervals,
workload version, metrics version — and an unversioned edit therefore breaks a comparison loudly
instead of passing quietly.

> **A name is a promise; a description is a measurement.** `STANDARD_CONTROL_V2` alone would let
> an edited protocol pass as the old one. The id carries the knobs, so it cannot.

**Process lifecycle turned out to be experimental apparatus.** The only named difference between A
and B was that B's server had just started and A's had already served a control. That is not proof
— it is a variable that was never normalised, and now is: `Fresh` means stop the server, load,
warm up, measure, let go, and the observation records what the process had actually done so *did
these two follow the same steps* is answerable after the fact instead of by recollection.

> **A control is only comparable if its lifecycle and warmup protocol are comparable.** *A first
> load is not a latency*, one layer out.

**Reproduction is equivalence, not "at least as fast".** A reference of 39.7 against a control of
46.7 passes a one-sided degradation gate — and it has not reproduced anything. `AboveReference` is
a real answer: the experiment changed or the reference is stale. It is also **not**
`RecoveryRequired`, because faster is not evidence of damage, and sending somebody to hunt a
hardware fault over it is the same invention as declaring the machine clean.

**Four numbers were sharing one word.** A calibration reported *"spread ±5.5%"* and the 5.5% was
the gate band, six MADs wide; the runs themselves spanned 2.97%, which the code correctly called
`Stable`. Nothing was wrong except the report — it conflated a description of the data with a
rule about the next reading, and on that basis a healthy calibration was very nearly thrown out.
Observed range, max deviation, MAD and allowed tolerance now cannot share a name.

**And two things were being persisted that were never identities.** The raw `--version` line,
`version: ` prefix and all, had leaked into a reference id — while an older record stored the same
build without the prefix. And a speculation setting was stored as `format!("{:?}", …)`, which
makes the `Debug` impl part of a record's meaning: add one field and every stored value silently
stops matching.

> **A display string is never an identity.** Persist the fields; build the sentence from them.

**A measurement is not a kind of measurement.** The store keyed references on the fingerprint, so
B overwrote A — two real observations, one survivor, and the only interesting thing about the pair
was that they disagreed. A fingerprint answers *what kind of experiment is this and what may it be
compared with*; a measurement answers *what happened this time*. Ids are monotonic now, history
keeps everything, and `latest_reference_for` is an index over it rather than a second store.

**Finally, a run does not become an authority by having happened.** `Observation → validate →
PerformanceReference`. Too few runs, or runs spanning more than one configuration, or a fingerprint
that could not record something a comparison needs: the observation is kept as evidence and the
authority is withheld. A refusal loses nothing.


## A reference can be right about its sources and stop describing the machine (added 2026-09-01, from six measurements)

`STANDARD_CONTROL_V2` was built to stop a benchmark comparing two different experiments. Then it
was used, six times in three hours, on one experiment: same binary, same artefact, same
fingerprint, fresh process each time with distinct router PIDs, protocol execution MATCH.

```
51.81 → 51.60 → 51.19 → 48.04 → 42.75 → 38.58
```

**−25.5%, monotonic, and ten minutes of real idle did not interrupt it** — the last reading came
after twenty-nine minutes with nothing loaded and fell another 9.7%. The card sat at 38 °C
throughout, so it is not thermal; shared memory grew 0.16 → 0.30 GB, two orders of magnitude below
the eviction event that this subsystem was originally written about, so it is not that either.

**No cause is claimed and that is deliberate.** Time, use, driver state, memory fragmentation and
twelve load/unload cycles of a 17.7 GB model all moved together, and none was controlled.

**What the instrument did was work.** `R-001` was minted from three calibrations agreeing to
within 1.2%, band 50.05–53.15. It then read `BelowReference` three times running, correctly. The
temptation was to widen the band until the new readings fitted — which would have deleted the only
thing that detected any of this.

> **Never widen a band to admit the evidence that contradicts it.** A gate adjusted until it
> passes is not a gate.

So the reference is marked `TemporalCoverageInsufficient` **and its numbers are never edited**. It
is a true summary of the three processes it was built from and a poor description of what the
machine does over hours.

> **A reference can be correct about its sources and stop describing the machine.** Those are two
> different failures, and only one of them is fixed by measuring again.

**Three properties, not one.** This machine is excellent at two of them:

| | what it asks | what was seen |
|---|---|---|
| within-process | is this instance steady while it runs? | 1.0–5.2%, steady |
| between-process | does a fresh load reproduce the last one? | 1.2% across three, in twenty minutes |
| **temporal** | does it still, hours later? | **25% across six, over three hours** |

A reference built only from the second is confidently wrong about the machine. The domain keeps
them apart; the policy for the third waits for evidence Epoch has not accumulated.

**And the route a measurement took must not decide whether it survives.** A control read 48.04
with five runs, a MATCH and a comparable fingerprint — the single reading that first showed the
reference was unrepresentative — and existed only as a sentence on a screen, because the code path
that took it was a comparison rather than a calibration. Its five rates could not be reconstructed
afterwards, and inventing them would have been the exact forgery this subsystem exists to prevent.
**Every valid standard control produces an observation now**, whatever errand asked for it.

**The operational consequence is worth more than the cause.** A search that opens at 51.6 and
reaches its third sentinel at 42.7 must stop and classify the state. Measuring twenty candidates
across that slope produces twenty rows where the last is a quarter slower than the first for
reasons that have nothing to do with any of them — and every one of those rows would look like a
finding.

## A rule kept by repetition is a rule that has already been broken (added 2026-09-07, from an audit)

An external audit read the code and scored it 48/100 for public release. Everything it found was
real, and the shape it kept finding is worth naming: **a property that was true because forty
places remembered to keep it true.**

**The clearest one.** EpochServices told the person's surface from the Host's by `if mine` on
every match arm. *The pages are not reachable from the network* was true, and it was true the way
a spelling is true — by being typed correctly forty times. The pages are now on a listener bound
to loopback and the Host's routes are in a different file. A rule the shape of the program keeps
is a rule nobody has to remember.

**And the oldest.** `capabilities/machine.rs` opened with **There is no shell**, reasoning that
injection was therefore not blocked but *inexpressible*. Sound reasoning about a program that no
longer existed: it began handing the line to a shell the day `git commit -m "two words"` had to
work, and that change documented itself two hundred lines down while the summary at the top kept
describing the version before it. The auditor read past the header and found the shell — which is
the cost of a stale comment in that exact place: somebody reads a guarantee stated as an absence
rather than a check, and stops looking for the check.

> **A comment that summarises code is a claim with no test attached.** The further it sits from
> what it describes, the longer it survives after becoming false — and the more load it carries,
> because distance is what makes it a summary.

**Four gates nothing ran had all already drifted:** 601 formatting hunks, 27 Clippy warnings, an
MSRV that said 1.82 while four dependencies needed 1.88 (so a machine with exactly the declared
minimum could not build this at all), and a dependency policy that did not pass. CI now runs all
of them, on both programs — EpochServices was not built there at all, which is the *two surfaces*
rule failing quietly for the third time.

### `false` and silence, once more, in the direction that looks responsible

Two of the audit's findings were the same inversion this file already has a section about, and
one was its mirror:

- **Loopback was read as identity.** A page in the user's browser reaches `127.0.0.1` exactly as
  a program's own window does, so *it arrived on loopback* was answering *who sent this*. Three
  conditions now, none leaning on another — where it came from, what it called us (`Host`, which
  is the DNS-rebinding gate), and whose page it is (`Origin`).
- **A checked address and a connected address were not the same address.** `vet` resolved a name,
  refused every internal answer and returned the host — under a comment claiming the caller
  therefore "cannot check one thing and fetch another". It could: `ureq` looked the name up a
  second time. The connection is pinned to the addresses that were approved.
- **And a ceiling that only bounded what was kept.** The body cap truncated the string and the
  channel behind it was unbounded, so how much this machine held was the caller's decision.

### No CSRF token, and that is a decision rather than an omission

It was in the recommendation. It would defend against a browser that omits `Origin` on a
cross-origin `POST`, which is a browser that does not exist. Forty forms carrying a hidden field,
plus a token to mint and rotate, to close a hole nothing can walk through, is the dead `Manual`
mode of ADR-0027 wearing a security hat. Where it would go is written down in `door.rs`.

> **A control that defends against nothing measurable is not defence in depth; it is a second
> thing to keep working.**

### Before blaming your own change, measure the commit before it

The Services page took 24 seconds to load with the new doors up. The obvious reading was the
refactor. Timed on the commit before it: 24.0 s. One `ureq::get` with a five-second `timeout` and
no `timeout_connect` — and the five seconds never start until there is a socket. **The rule had a
section in this file already, with this exact number in it**, applied to the Bridge and to
`look_for` and never to the one call that program makes on its own. 24.0 s to 3.5 s.

### Verification is not the same errand as the work

Every security claim in this session was checked against the running program with `curl` — a
spent code refused the second time, a page route absent rather than forbidden, an unpair from a
foreign `Origin` refused with the bond surviving, a 2.7 MB body answered `413`. 11.18 is why:
**a harness that declares its own conditions measures a path the product does not take.**

## The other machine is where a Windows-shaped truth stops being true (added 2026-09-07, from running it there)

The security work was finished, measured and committed on Windows. Then the Mac was switched on
and asked the same questions, and it found three things nothing here could have.

**The application did not compile on macOS at all.** `epoch-tauri` calls `machine::driver()`,
and `driver()` carried `#[cfg(not(target_os = "macos"))]` — an attribute that belonged to the
function *above* it and had drifted one item down, landing between `driver()`'s own two doc
comments. One of those comments says, in as many words, *`None` on every machine that will not
say, **including every Mac***. The doc described the intent and the attribute contradicted it,
and the attribute is the half the compiler reads.

The same shape as the four found in `4998365` and now the most expensive instance of it: an
attribute that survived an edit its code did not, sitting in a place only another operating
system could reach.

**A door that reported its failure to its own thread.** `wait_for_one` returned `Ok` and *then*
bound the port on the thread it had just spawned — so a taken port was reported to that thread
and the caller was told the door was open. That is a race rather than a state, and the two
machines disagreed about which way it went: on Windows the listener was up before the first
connection and on macOS it was not, so two tests that had passed for a day failed there with
`connection refused`. Binding is the caller's own step now.

> **Whoever must hear about a failure has to be on the thread that can fail.** Spawning first
> and binding second turns a `Result` into a coin toss, and the coin is weighted differently on
> each operating system.

**And serialising the tests was necessary and was not the fix.** Two doors on one port produced
`connection reset` — one test's client reaching the other's listener — so a lock came first.
The lock changed which half of the race lost, and the real defect was still there.

**And then the nine that were left.** They were not one problem, and only one of them was a
test:

- **One was the product.** The installer offered whisper.cpp with a live button and no bytes:
  the project publishes an xcframework for macOS rather than a runnable binary. A box that can
  only fail is the invented gauge wearing an install button. It is dark now and says which
  absence it is — and the voice-chain test grew the other half of its rule rather than a `cfg`:
  **offered with a way in, or offered, dark, and explained.**
- **One was a rule with two spellings.** `confine` refuses `new/../../../elsewhere.rs` on both
  machines and gave two different reasons, because Windows normalises `a/../..` lexically
  before touching the filesystem and Unix does not. A `..` is refused as it is walked past now.
- **Four were fixtures that measured nothing while reading as though they measured the most
  important case.** `C:/Windows/System32` is an absolute path on Windows and an ordinary
  relative folder name on Unix; `..\..\x.png` is a path on one and a filename on the other. Each
  asserts the shape of the machine it is running on, and the Windows-only ones say so.
- **Three were the machine being asked at the wrong moment.** macOS's login keychain is locked
  until somebody signs in *at the machine*, so `security` answers `User interaction is not
  allowed` over SSH. `Store::reachable` gives three answers rather than two — **Yes**, **Locked**
  (nobody is there) and **Broken** — and only *locked* skips a test. Collapsing locked into
  `false` is the same inversion this file keeps paying for.

> **A suite that passes on one operating system has been measured on one.** Both surfaces built
> in the same session is not enough when both builds happen on the same machine.

**1685 on Windows, 1671 on macOS, both green** — the difference is what `#[cfg(windows)]`
genuinely means. A macOS CI is possible now, and it was not this morning.

**The general rule, and it is the two-surfaces rule with a platform instead of a program:** a
suite that has only ever run on one operating system is a suite that has only ever measured
one. Building both surfaces in the same session is not enough if both builds happen on the same
machine.

## What replaces a signature when there is no money for one (added 2026-09-07, from measuring the downloads)

Epoch ships unsigned. A code-signing certificate is a recurring cost, an Apple Developer account
is another, and neither changes anything the program does — they remove a warning. So the
question is not *how do we afford a signature*, it is **what was the signature actually going to
prove, and which half of that is free.**

A signature proves two things at once, and they are usually confused:

| | costs money | free |
|---|---|---|
| *who made this* | a certificate, per year | — |
| *this file is the one that was built, from this source* | — | checksums **and** build provenance |

The second is the half that matters when a stranger downloads a binary, and GitHub gives it
away: `actions/attest-build-provenance` signs a statement through Sigstore, into a public
transparency log, saying the artefact came from this repository at this commit on GitHub's own
runners. `gh attestation verify` checks it. **A file built on the author's desktop has no
evidence but the author's word**; this replaces that with something anybody can check.

**And it does not stop SmartScreen.** Nothing but reputation or an EV certificate does, and a
README that implied otherwise would be the invented gauge in prose. So both READMEs say plainly
what each platform will show and how to get past it — and the honest version of *unsigned* is
not an apology, it is an instruction plus a way to verify.

**Measured before any of it was written.** The Windows installer reads `NotSigned`; the macOS
bundle is `adhoc, linker-signed` with no team identifier and `spctl` refuses it. And the copy
installed on the developer's own Mac carries **no `com.apple.quarantine`**, because it was built
there — so every test of "does it open" had taken a path no downloader takes. That is the
harness-declaring-its-own-conditions failure (11.18) wearing an operating system.

## A harness that waits is a harness that lies (added 2026-09-08, from the wire)

Gemini CLI was the last brain that never asked Epoch before writing a file, and the fix was the
Agent Client Protocol — `gemini --acp`, `session/request_permission`, Epoch's own approver
answering. The protocol had been measured from Python a session earlier and pronounced proved.
Building it found two defects, and **neither was reachable from that probe, from 1148 tests, or
from reading the code.** Both needed the actual bytes, which is why `EPOCH_TRACE_ACP` is still in
the file.

**A probe that sleeps between two messages has measured a sequence the product does not send.**
Epoch wrote `session/set_mode` and `session/prompt` back to back; the agent answered the mode and
sometimes dropped the prompt without a word. One run reached the permission question in 13 s and
the next never answered at all — a race, so the first run "passing" proved nothing. The Python
probe had `time.sleep` between the two lines, and that sleep was doing the work nobody had
noticed it was doing.

> **11.18 said a harness that declares its own tools measures a path Epoch does not take. This is
> the same failure in the time dimension: a harness that *waits* measures a protocol Epoch does
> not speak.** The instrument is part of the measurement — and a `sleep` in a probe is an
> instrument.

The safety half is worth more than the hang: `Manual` means *ask me*, and a prompt racing the
mode it depends on could have run under whatever the session was set to before.

**And a killed shim is not a stopped program.** The turn answered, printed its last word, set
`done` — and never returned. `Child::kill` ends `gemini.cmd`; the real program is `node
gemini.js`, which re-executes itself with a larger heap, so the grandchild outlived the kill
holding the reader thread's pipe, and `join()` waited forever while the crew card read `WORKING`.
`capabilities::machine` had found the identical fact about a *shell* months before and written a
careful comment about `taskkill /T` — privately. It is `epoch_models::quiet::stop_tree` now.
**A rule kept by repetition is a rule that has already been broken**, and the second place to
need it is where you find out.

**And the third one shipped in the installer, because every test had handed it a clean path.**
With the transport working and the live test green, the first turn in a real World failed at
`session/new`: containment canonicalises a Project Root, so Epoch was sending
`\\?\C:\Users\…`, and Node splits that into a root of `\\?\` and a first segment of `C:`
— `EISDIR: illegal operation on a directory, lstat 'C:'`. `library::plainly` exists for exactly
this, says so in its own doc comment (*"the resolved form is exactly what must never be assumed
to be presentable"*), was written for Obsidian months earlier, and no agent adapter had ever
called it.

> **A fixture that happens to supply clean input measures the wiring and nothing else.**
> `std::env::temp_dir()` carries no prefix and a Project Root always does, so the one test that
> ran the whole transport end to end was testing a path the product never takes. It
> canonicalises now.

That is the same failure as a harness declaring its own tools (11.18) and a harness that sleeps
(above), from a third direction — the **data** this time rather than the tools or the timing.
Three shapes, one rule: **whatever the test supplies for itself is the part it is not
measuring.**

## A list of somebody else's models is a measurement (added 2026-09-08, from asking)

Epoch offered six Codex models, four Claude Code aliases and nothing for Gemini, all compiled in.
Asked on this machine the same day, Codex offers **`gpt-6-astra`** — not on the list — and no
longer offers `gpt-5.4`, which was. So a character could be pointed at a model that does not
exist and fail inside a Quest instead of at the door, and a person could not reach the best model
their own subscription pays for.

**Both programs answer, and neither says so in `--help`.** Codex has `model/list` over the same
app-server the allowance already uses; Gemini names them in `session/new`'s own reply. Same
discipline as ADR-0027's three facts and 11.18's `dontAsk`: *somebody else's flag is a
measurement, not a memory* — including the absence of one.

~~**Claude Code genuinely has no listing**, measured rather than assumed.~~ **Wrong, and
corrected the same day.** `-p /model` prints its own vocabulary for nothing — `num_turns: 0`, no
tokens, no API time, 94 ms — and names **ten** values where Epoch's compiled list had four:
`best`, `opusplan`, `default` and the long-window `sonnet[1m]` alongside the aliases.

> **A measurement answers the question you asked.** `--help` was asked *which subcommands exist*
> and answered truthfully; the question that mattered was *what does a slash command answer in
> `-p`* — and `claude.rs` had been reading `/usage` exactly that way for months, three hundred
> lines from where the wrong conclusion was written.

The correction is left visible rather than tidied, because the sentence it replaces was recorded
here **as a measured fact** and would have been trusted. The rule it breaks is the one this file
already has: *re-measure the entry you are about to act on*. This one was acted on.

What survives from it: the **aliases** really do track the latest of each family — measured,
`--model sonnet` reports `claude-sonnet-5` in `modelUsage` — so a stale fallback still works. And
a brand-new *family* nobody has shipped is still answered by the picker's "name it myself" field
rather than by probing: **a control assembled from what is conceivable will offer combinations
that do not exist.**

**And the compiled list did not go away; it stopped being the answer.** A program that is not
installed, not signed in, or too old to speak the protocol keeps its suggestions, and the field
stays typeable. Replying *there are no models* to a question nobody could put is the inversion
this file has now paid for five times.

**A correct list can still be an unreadable one** (added 2026-09-08, from two screenshots). The
owner put Claude Code's own picker beside Epoch's: *Fable 5.1 · Opus 5 · Sonnet 5 · Haiku 4.5*
against `fable · opus · sonnet · haiku`. Both were right. Only one could be acted on — the fifth
face of the cold-instrument rule, arriving in a list this file had congratulated itself on two
sections earlier.

The fix was another free question to the same program. `-p /model` reports the **session's**
model, so `--model opus` alongside it makes it answer `Current model: Opus 5` — `num_turns: 0`,
no tokens, no API time, ten of them in 5.4 s in parallel. An alias is what you type; the model is
what you get; **the program will say which, and it costs nothing to ask.**

*And it stays true by itself*, which is the whole point: the day a newer model ships, the CLI
resolves the same alias to it and Epoch says the new name with nothing changed here. A table of
`opus -> Opus 5` inside Epoch would be a sentence about a moment, wrong the morning after — which
is exactly what the fallback's old *"Opus 5 — the latest"* labels were.

**What this does not fix, and the difference is worth keeping straight.** The owner's picker
showed `Fable 5.1` and Epoch could not: the CLI on that machine was 2.1.247 and did not know it.
Epoch reports what the installed program knows, and a list that invented a model the local CLI
would refuse is the failure this whole section exists to avoid. *Somebody else's version is a
measurement too.*

**Closed the same hour, by them running `claude update`.** 2.1.265 landed and `Fable 5.1`
appeared in Epoch's picker with **no code change** — which is the mechanism proving itself rather
than a claim about it.

And the new version immediately paid the rule back: it writes Current model: `Fable 5.1` ,
with backticks the old one did not use. One afternoon, one machine, two shapes from the same
command. They are trimmed rather than matched, so a version that stops using them needs no edit
either, and the fixture keeps **both** — a parser that only knows the newer shape breaks for
everybody who has not updated yet.

## A guard keyed on a name protects the name (added 2026-09-10, from testing an update)

The published v0.1.0 was installed, a World was made in it, and a newer build was installed on
top. The update kept the World. Measuring *why* found three things nobody had reason to look for.

**User data was in a folder somebody else owns.** A new World was written beside the binary, into
the installer's `packs`, while the table in `paths.rs` said Worlds live in the vault. It survived
the update because the uninstaller removes that folder with a plain `RMDir`, which refuses a
folder that is not empty — and the World was what made it not empty. A plain uninstall then left
it in a program folder with no program. **Surviving by accident is not surviving:** the first
installer that tidied up with `/r` would have been a data-loss release with no change to Epoch at
all.

**A guard keyed on a name protects the name.** `erase::SHIPPED = "default"` existed to stop the
shipped World being deleted. `default` stopped shipping and the constant did not move, so for two
days it protected nothing, while REMOVE on the Archipelago planned to delete its manifest —
measured through the real function, not by pressing the button. Correcting the string would have
been the same bug waiting for the next rename. The fix removes the possibility instead: the
shipped folder is no longer an argument to removal, so nothing can name a file in it, and the one
place that still needs *what shipped* reads it off the folder the installer bundles.

> **Prefer making the protected thing unreachable to excluding it by name.** An exclusion list is
> a second copy of a fact, and the copy is the half nobody updates.

**Moving data is a privacy change, not a path change.** Once a World's pack and its vault folder
became one folder, the export that took *the pack, whole* would have archived every conversation
in it. Nothing would have failed; it would simply have sent them. A test now builds exactly that
folder and asserts the archive holds the manifest and the map and nothing else.

**And a comment is code when another tool compiles the file.** Correcting a comment in the NSIS
hook broke the installer: a heredoc halved a backslash, `\v` reached Python as a vertical tab, and
makensis read it as a line break. The build chain used `;`, carried on, and built `epoch-setup`
around the previous installer — whose *carries the installer* check then compared a stale file
with itself and printed `True`. Chain builds with `&&`, and check an artefact is newer than the
build that claims to have made it.
