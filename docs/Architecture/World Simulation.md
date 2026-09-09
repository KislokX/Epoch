# World Simulation (Presence)

> Status: Designed · Owner ADR: [[../ADR/0018-world-simulation]] · Horizon: mixed (see ADR)
> Principles: **The Engine owns reality; the UI owns animation.** · **A Snapshot preserves continuity; it never defines reality.**

The bridge between the Engine and the Living World. An **Engine subsystem** (not UI) that maintains observable presence as a **deterministic projection of real engine state**.

## Purpose
Make the Living World possible without letting it become fiction. Every character position, every building state, every visible activity traces to something real.

## Separation of concerns
| Concept | Describes | Owner |
|---|---|---|
| `CharacterDefinition` | identity (immutable) | [[Definition Runtime]] |
| `CharacterInstance` | execution state | [[Definition Runtime]] |
| **`PresenceProfile`** | routine, idle behavior, home place, movement affinity — *authored* | **this subsystem** |
| **`PresenceState`** | current place, destination, progress, speed, ETA, current activity — *derived* | **this subsystem** |

Presence left `CharacterDefinition` entirely (resolves the gap deferred in ADR-0011). Identity and presence are separate contracts.

## Simulation rules
- Characters never teleport.
- Characters always have a current place.
- Characters have routines, idle states and destinations.
- Characters react to **real** Activities.
- Buildings expose their current state.
- Conversations happen somewhere.
- Presence is observable — *"where is the Researcher?"* is a real query, answerable headless.
- Time advances.

## The causality rule
**Every presence transition traces to a real cause:** an Activity, a Character Instance state change, or a declared routine rule.

No autonomous NPC AI. No emergent wandering. No fabricated conversations. No decorative behavior. The simulation is a *function of real state*, never a generator of life.

Routine-driven movement while idle **is** legitimate — "available" is true information — but idle movement must be **visually distinguishable** from working movement ([[../../LIVING_WORLD_DESIGN_GUIDE]]), or the World becomes unreadable.

## Time and publication
The Simulation owns time and advances every Presence. It publishes **event-driven, meaningful updates only** — never per frame:

```
StartedTravelling · ProgressUpdated (coarse) · Arrived
ChangedDestination · StartedWorking · BecameIdle
```

`PresenceState` = Current Place · Destination · Progress (0..1) · Speed · ETA · Current Activity.

The UI interpolates smoothly between two authoritative states. A client joining mid-journey immediately receives the current authoritative state. **No client ever invents reality.**

Gives us: headless compatibility · multi-client agreement · low event traffic · smooth animation · real presence queries · future multiplayer left open.

## Communication
- **Consumes** the [[Activity Stream]] (real Activities drive reactions) and Instance state from [[Definition Runtime]].
- **Emits** presence Activities; the UI subscribes and renders. [[Knowledge Engine]] may derive from them.
- Uses canonical **place concepts** ([[World Packs]]) — `knowledge_center`, `research_lab`, `automation_hub`, `command_center`, `guild` — never location names.
- The World Pack **renders** presence; it never **determines** it.

## NOT responsible for
- Inventing behavior or running autonomous AI.
- Being a source of truth (repositories are — [[Persistence]]).
- Rendering (the UI) or deciding what things are called (the World Pack).
- Identity (Definitions) or execution (Instances / [[Execution Engine]]).

## Persistence
**Snapshot** category ([[Persistence]], ADR-0014). Captures *where the projection currently is*, not the world itself. If a snapshot is missing, lost or incompatible, the Simulation **reconstructs from canonical state and continues**.

Repositories = source of truth. Activities = historical truth. Definitions = identity. Instances = execution. The snapshot is only an optimisation for continuity, and is always disposable.

## Absence
The engine does not run while the application is closed, so **the World is not simulated during absence.** "While you were away" is **reconstruction** from real Activities and Knowledge produced by scheduled Quests or background runs — never simulated backfill.

## Horizon
- **NOW:** PresenceProfile + PresenceState contracts, simulation clock, causality rule, Activity consumption, event-driven publication, place occupancy, snapshot/restore.
- **DESIGN NOW:** richer building/place state, conversation-location semantics, multi-client sync.
- **VISION:** multiplayer, NPC populations beyond the archetypes.

## Risks
- Drifting into a game engine with autonomous AI → the causality rule; every transition names its cause.
- Idle movement misread as work → mandatory visual distinction.
- Tick cost / event storms → coarse, event-driven publication.
- Snapshot incompatibility → disposable by rule; reconstruct and continue.

## Open questions
- Routine rule expression format (declarative schedule vs. simple state machine).
- Whether `PresenceProfile` is per-archetype, per-Instance, or both.
- Building/place state vocabulary richness.
- Coarse-progress publication cadence.
