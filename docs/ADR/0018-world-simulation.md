# ADR-0018: World Simulation (Presence)

- Status: Accepted
- Date: 2026-07-25
- Depends on: [[0003-engine-presentation-separation]], [[0011-definition-runtime]], [[0014-persistence-contract]], [[0015-activity-stream]], [[0017-character-archetypes-world-packs]]
- Resolves: the Presence schema gap deferred from [[0011-definition-runtime]]
- Related architecture: [[World Simulation]], [[../UX/World]], [[../../LIVING_WORLD_DESIGN_GUIDE]]
- Horizon: PresenceProfile + PresenceState contracts, simulation clock, causality rule, Activity consumption, event-driven publication, place occupancy, snapshot/restore **IMPLEMENT NOW**; richer building state, conversation-location semantics, multi-client sync **DESIGN NOW**; multiplayer, NPC populations **VISION**

## Context
The Engine spine is complete and three constitutions now *require* a missing contract: the Living World Design Guide's movement rules, the Character Bible's routine/idle/home place, and the Experience Manifesto's "characters keep working". The Presence fields (Routine, Idle behavior, Home place, Relationships) were twice deferred from `CharacterDefinition` under Earn Complexity. That complexity is now earned — this is the bridge between the Engine and the Living World, and without it every animation would be a client-side invention.

## Decision
Introduce the **World Simulation**: an **Engine subsystem** that maintains the observable presence of runtime entities as a **deterministic projection of real engine state**.

### Two new standing principles
- **The Engine owns reality. The UI owns animation.** Animation never creates information; it only visualises information already produced by the Engine.
- **A Snapshot preserves continuity. It never defines reality.**

### Separation of concerns
| Concept | Describes | Owner |
|---|---|---|
| `CharacterDefinition` | identity (immutable) | Definition layer (ADR-0011) |
| `CharacterInstance` | execution state | Definition Runtime (ADR-0011) |
| **`PresenceProfile`** | routine, idle behavior, home place, movement affinity — *authored* | **World Simulation** |
| **`PresenceState`** | current place, destination, progress, speed, ETA, current activity — *derived* | **World Simulation** |

Presence leaves `CharacterDefinition` entirely. Identity and presence remain separate contracts.

### Simulation rules
- Characters never teleport.
- Characters always have a current place.
- Characters have routines, idle states and destinations.
- Characters react to **real** Activities.
- Buildings expose their current state.
- Conversations happen somewhere.
- Presence is observable (queryable: *"where is the Researcher?"*).
- Time advances.

### The causality rule (the guardrail)
**Every presence transition must trace to a real cause:** an Activity, a Character Instance state change, or a declared routine rule. No autonomous NPC AI, no emergent wandering, no fabricated conversations, no decorative behavior. The simulation is a function of real state — never a generator of life.

Routine-driven movement while idle **is** legitimate: "available" is true information, and the Design Guide permits ambient life. Idle movement must be **visually distinguishable** from working movement, or the World becomes unreadable.

### Time and publication
The Simulation **owns time** and advances every Presence. It publishes **event-driven, meaningful** updates only — not per frame:

`StartedTravelling` · `ProgressUpdated` (coarse) · `Arrived` · `ChangedDestination` · `StartedWorking` · `BecameIdle`

`PresenceState` carries: **Current Place · Destination · Progress (0..1) · Speed · ETA · Current Activity**. The UI interpolates smoothly between two authoritative states. A client joining mid-journey immediately receives the current authoritative `PresenceState`. **No client ever invents reality.**

### Persistence
World state is **Snapshot** category (ADR-0014) — it captures *where the projection currently is*, not the world itself. If a snapshot is missing, lost or incompatible, the Simulation **reconstructs from canonical state and continues**. Repositories remain source of truth; Activities remain historical truth; Definitions remain identity; Instances remain execution.

### Absence
The engine does not run while the application is closed, so **the World is not simulated during absence.** "While you were away" is **reconstruction** from real Activities and Knowledge produced by scheduled Quests or background runs — never simulated backfill.

## Why this is best
- Makes the Living World possible without letting it become fiction: every visual traces to real state.
- Engine-authoritative presence keeps headless viable (`where is X?` works with no UI) and keeps multiple clients in agreement.
- Event-driven coarse updates give smooth animation at low traffic.
- Gives the deferred Presence data a home without polluting identity.
- Snapshot-as-continuity preserves "nothing is temporary" while creating no second source of truth.

## Alternatives considered
- **Simulation in the UI** — per-client fiction, headless impossible, clients disagree. Rejected.
- **Discrete state, UI interpolates freely** — intermediate position becomes a client invention. Rejected.
- **Per-frame state streaming** — authoritative but heavy traffic for no gain. Rejected.
- **World state as Persistent (canonical)** — contradicts presence being derived; creates a second source of truth. Rejected.
- **Ephemeral, rebuild every launch** — everyone reappears at home; reads as inter-session teleportation. Rejected.

## Consequences
- The Engine gains a clock/tick loop — its first genuinely time-driven component.
- `PresenceProfile` becomes authored data alongside Definitions; place values use canonical place concepts (ADR-0017), never location names.
- The World Pack renders presence; it never determines it.
- The UI's contract narrows usefully: render + interpolate, never decide.

## Assumptions
- Coarse progress updates plus ETA are sufficient for smooth client-side interpolation.
- Routine rules can be expressed declaratively without scripting.

## Risks
- **Drifting into a game engine with autonomous AI.** *Mitigation:* the causality rule; every transition names its cause.
- **Idle movement misread as work.** *Mitigation:* mandatory visual distinction (Living World Design Guide).
- **Tick cost / event storms.** *Mitigation:* coarse, event-driven publication; no per-frame state.
- **Snapshot incompatibility.** *Mitigation:* snapshots are disposable by rule; reconstruct and continue.

## Open questions
- Building/place state vocabulary richness (DESIGN NOW).
- Movement affinity: one walking speed serves everybody until somebody has a reason for a
  character to be quicker than another. Inventing a difference before then would be the World
  making up a fact about a person.

---

## Amendment - 2026-08-09, from implementation

The clock, travel and the causality rule were built. Three of the four open questions are
answered by what the build revealed rather than by argument, and two things the ADR did not
anticipate turned out to be load-bearing.

### Answered: the routine rule format is a schedule, and *where* is not part of it

A declarative cycle of activities was enough. No state machine, no scripting, no conditions:
each behaviour has a name and a length, and which one is current is elapsed time against the
cycle. That is the whole expression format, and it is what keeps the causality rule cheap to
hold - there is nothing in the data that could branch.

**But where a behaviour happens is not the character's to say.** It was written onto
`IdleBehavior` first, and that reproduced exactly the mistake ADR-0028 caught for `home`: a
`PlaceId` means something only inside one World. `library` is a different building in every World
and in most of them does not exist at all, so a routine naming one would send the same person to
the right door in one World and to a stranger's in another.

So the split is the same one identity always takes here. *Reading for sixty seconds* is who
somebody is and travels with them; *reading happens at the Library* is a fact about a World and
lives in `Residence.at`, beside `home`. Unlisted means home - which is what every character
described before a routine could name a Place at all, so a vault written for the old shape
behaves identically.

### Answered: `PresenceProfile` is per-Definition, and its per-World half is a Residence

Not per-archetype, and not per-Instance. The Instance *snapshots* both halves at spawn, like
everything else it reads from a Definition.

### Answered: publication cadence is a tenth of a journey, on its own channel

A tenth of the way is close enough that a correction never reads as a jump and rare enough that
a thirty-second walk costs about a dozen messages. But the cadence was the smaller half of the
problem: `world:changed` carries every Place, every mark and every face, and it was the only way
the World heard about anybody. Movement therefore has **its own channel** carrying only who
moved. The full projection is still what arrives when the World's *shape* changes - who, where,
doing what, bound for where - because those are the moments a surface may need more than a
position.

### New: a walk in progress is never cancelled; only its purpose changes

Not in the original ADR, and it should have been, because "characters never teleport" is not
only about arrival. Two implementations that read as correct were teleports:

- work starting "where they are standing" ended an in-flight walk and put the character back at
  the Place they set out from;
- a Quest finishing before its contributor arrived snatched them back the same way.

On screen both are a figure halfway down a road vanishing and reappearing at a door. **The reason
for a journey can end; the journey cannot.** A second destination therefore *queues*: somebody
already walking starts the next leg from where that road ends, because where they are standing is
a point on a line and the only thing a leg can begin at is a Place.

### New: a walk that would outlast its reason does not begin - except going home

Ten seconds of walking against a six-second behaviour is somebody who turns around every time
they get halfway: caused, honestly derived, and unreadable. So a walk begins only if it fits
inside the behaviour that asked for it, and the author can see why it did not.

**Home is exempt.** It is not a destination behaviour - it is where the routine lives - and held
to the same rule, somebody who walked across the map to take over a Quest would be stranded there
for good the moment their behaviours were shorter than the walk back. That reads as broken rather
than as careful.

### New: scenery may not veto work

A Place this World does not have, or has never positioned, is not an error. The work starts where
the character stands. An unbuilt World gets on with it, which is the same rule `places.rs`
already stated for roads.

### Measured rather than chosen

- **Walking speed** is 60 world units per second, from the shipped map: 3600x2200, furthest two
  buildings about 1750 units apart, so the longest walk in the default World is just under thirty
  seconds.
- **The clock is a parameter** everywhere it is read. The first version called `Instant::now()`
  inside `work_at` while `advance` was handed a timeline, and a test that departed at `start` and
  arrived ten seconds later missed by the microseconds between the two reads.

### What the road carries

`places.rs` has said since ADR-0028 that a road carries work: when a Quest is handed from
somebody in one building to somebody in another, the World answers by walking it. That sentence
now executes, and only on a handoff - the most explicit of the three things allowed to move a
Quest (ADR-0025 SS7b). Somebody the user simply typed to works where they are: they were asked
something, not sent somewhere.

## Amendment (2026-08-19): the action, beside the sentence

ADR-0016 named `character.architect` + `walk.east` as canonical concepts on the day it was
written, and the Engine has never had the first half of that pair. Presence published a
*sentence* — `"walking to The Library"`, `"thinking with gemma4:12b"` — assembled by `format!()`
in a dozen places. No renderer can turn that into frames.

`PresenceState` now carries **`Action`** beside `activity`: a closed enum, `Idle · Walk · Think ·
Work`.

**Set at the call sites, never parsed back out of the sentence.** Deriving it from the words
would make two authors of one truth, which is the defect this project removed from four other
places in the same month. The sentence stays: the two answer different questions — *what do I
draw* and *what do I tell a person who is reading*.

**Beside `class`, never instead of it.** `class` is the honesty rule (Build From Life 3: routine
may never look like work that is not happening); `action` is the animation. Walking to work is
`class: Work` and `action: Walk` at the same time, and collapsing them would put a renderer in
charge of a distinction the Engine is supposed to own.

### `Think` is a state, and the first tool ends it

*Nothing is happening*, *the character is reasoning* and *execution has begun* are three
different facts, and until now the World could only tell the first from the other two.
`start_working` opens a turn as `Think`; `began_running` promotes it to `Work` in place —
same turn, same journey if there is one — and rewrites the sentence, because *"thinking with
gemma4:12b"* stopped being true.

**Two steps promote it, because the two brains report at different moments.** Epoch's own
capabilities announce `Step::Using` *before* they run, which is the honest instant. An Agent
runs its own tools and Epoch only witnesses them afterwards (`CLAUDE.md`, 2026-08-07), so
`Step::Used` is the earliest this side can know — still true, still caused, just later.
Inventing an earlier one would be a surface fabricating timing.

**Nothing promotes somebody idle.** A tool running for a character who was never put to work
would be a transition with no cause on this side of it; the cause of work is a turn starting.

### Why an `Effort`, and not the enum itself

Only two of the four are things a caller may *start*. `Idle` comes from the routine and `Walk`
from a leg — both decided inside the Simulation. So the public parameter is `Effort {
Thinking, Running }`, and asking somebody to start walking without a road is not a sentence that
can be written.

### `Sleep` is deliberately absent

Nothing causes it. There is no night, and a sleeping character with no night to explain them is
the autonomous NPC behaviour this ADR forbids. It arrives when its cause does — as will `Talk`
and `Settle`, which need the arrival beat that causes them.

## Amendment (2026-08-19): a journey knows which way it points

A directional sheet has one row per direction, and something has to choose the row. The
coordinates are on screen, so a renderer *could* work it out — and that is exactly the mistake
this ADR names first in another form: a client deciding something about the World from what it
happens to be drawing with.

So `Journey` carries **`facing`**, measured once at departure with the geography in hand.
Whichever axis the leg covers more of wins, and a perfect diagonal falls to the horizontal
because a side-on walk reads better than a back or a front at the same angle. Four, because four
is what a sheet distinguishes.

**It lives on the journey, not on presence.** A journey is the only thing that has ever caused
somebody to face one way rather than another; a standing character has no measured facing, and
inventing one would be the same lie one layer down. A directional sheet with no facing plays in
reading order instead of being pinned to a direction nobody measured.

### The gait is a fallback now, not a decoration

The World laid a CSS bob over every walking figure. With authored artwork that bob is somebody
else's rhythm on top of a gait the sheet already contains, so it is skipped the moment a sheet
is playing — and kept, exactly as it was, for everybody nobody has drawn walking. **The bob *is*
the walk when there is no sheet**, which is what makes it a fallback rather than something to
delete. Authored artwork wins; the derived thing stays for the World that has none (ADR-0024).

### One cadence, two ways of drawing

The editor draws a sheet as a CSS background in a box; the World draws it as an `<image>`
clipped inside SVG. They share the *timing* — one hook — because a walk that looked right in the
editor running at a different speed outside it would be two answers to one question.

## Amendment (2026-08-19): arriving is something that happens

A figure went from walking to working in one frame. Nothing was wrong with it — presence was
accurate at every instant — and it read as an animation switching rather than as somebody
deciding. So arrival is a state now:

```text
routine:   Idle → Walk → Arrival → Settle → Idle
handover:  Idle → Walk → Arrival → Talk   → Think → Work
```

`Settle` and `Talk` are **beats, never states.** They land, and then become whatever the journey
was for. A `Talk` that persisted would imply a conversation in progress that is not happening —
the one thing the Living World Design Guide names outright — and no words pass between
characters, because ADR-0023 forbids one writing another's lines.

### The branch is the whole point

`Talk` is *the person the work came from is standing here*. A handover walks the receiver to the
last contributor's Place, so two characters together **because the work passed between them** is
measured rather than inferred. With nobody there it is `Settle`: the same beat without the
second person.

**Asked on arrival, never at departure.** The giver may have walked somewhere else while the
receiver crossed the map, and that case is exactly what separates the two. Checked at departure
it would be a promise; checked on arrival it is a fact.

**And "here" means standing here.** Presence keeps a traveller at the Place they set out from
until they arrive — the honest answer to *where is she*, and the wrong answer to *is she here*.
Somebody halfway down the road is not somebody you can arrive to, so the arrival asks only about
people who are not travelling. A `Talk` drawn at a departing figure would be the World showing a
meeting that is not happening, which is the failure this whole distinction exists to prevent.

### The beat never delays work

Presence during the beat is still **work-class** when the journey was for work: the turn started
when it started, and this is presence catching up with a figure that has just stopped moving.
Build From Life rule 3 is about never dressing routine as work; it is not a licence to pretend
work stopped. And nothing may cut a beat short — a purpose changing mid-beat replaces what
happens *after* it, exactly as a purpose changing mid-walk never cancels the walk.

`BEAT` is 1200 ms and is **chosen, not measured** — worth saying, because nearly every other
number in `simulation.rs` is measured. It is long enough to read at sixty frames a second and a
twentieth of the longest walk in the shipped World.

### One consistent instant

Everybody's position is read **before** anybody advances, so an arrival asks about a World at one
instant rather than one where the answer depends on who was ticked first.
