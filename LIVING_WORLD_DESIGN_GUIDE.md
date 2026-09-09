# Living World Design Guide

> A design constitution — **not** an ADR, not implementation.
> The [EXPERIENCE_CONSTITUTION.md](EXPERIENCE_CONSTITUTION.md) explains **what** the World should feel like.
> This guide explains **how** we achieve that feeling.
> It is the visual and behavioural constitution of Epoch — the World's body language.

---

## The one rule above all

**The engine drives the World. The World never lies.**

Every visual originates from the engine — the Activity Stream (ADR-0015), real Instances (ADR-0011), real Knowledge (ADR-0010), real repositories (ADR-0014). If nothing is happening, the World does not pretend something is.

The simulation is a **projection of reality**, not an illusion. A fake animation is worse than no animation, because it destroys the only thing that makes the World worth trusting.

---

## Movement

**Characters never teleport.** Position is state; changing it takes time and is visible.

**Characters travel through the World.** If the Researcher needs to reach the Historian, she walks there. The journey is information: it tells the user that a handoff is happening, and roughly how far along it is.

**Movement has intent.** Characters never wander randomly. Every path has a destination, and every destination has a reason grounded in real work.

**Movement is readable at a glance.** The user should understand what is happening from silhouette and direction alone, before reading any text.

---

## Places

**Conversations happen in places.** A Chronicle is not a floating window — it occurs somewhere, and where it occurs carries meaning.

**Every building represents a subsystem.** Library → Knowledge Engine. Laboratory → Models & Integrations. the Guild → Party Builder. Command Center → active Era. Automation Hub → Automations. the Settings hall → Settings.

**Buildings communicate state.** A busy Library looks busy. An Era under active work looks active. Lights, activity level and occupancy reflect real engine state — never decoration.

**Every location has purpose.** If a place exists, the user can do something there or learn something from it.

---

## Animation

**Animation communicates information before text.** The user should know *something happened* from motion, then learn *what* from the detail.

**Show the work, not a spinner.** the Guardian debugging looks like the Guardian debugging. the Coordinator implementing looks like the Coordinator in the Workshop. the Historian documenting looks like the Historian in the Library. Reserve abstract progress indicators for cases where the work genuinely has no world-representation.

**Animation is observability.** Every animation maps to an Activity. Reading the World should be equivalent to reading the Activity Stream — just far more pleasant.

**Nothing animates without meaning.** Ambient life is allowed (breathing, idle sway, weather) because *the World being alive* is itself true information. Ambient life must never imply work that isn't happening.

---

## Silence and stillness

**Silence is also information.** A quiet World means nothing is running. That is a true and useful signal — do not fill it with noise to seem busy.

**Idle time is meaningful.** An idle Character is available. A missing Character is busy elsewhere. Both states are readable, and both are true.

**Stillness must be distinguishable from frozen.** An idle World still breathes; a broken World does not. The user must never confuse "nothing to do" with "something is wrong."

---

## Presence

**Characters are inhabitants, not widgets.** They have location, state, goals, current activity, routine, relationships and memory (see [CHARACTER_BIBLE.md](CHARACTER_BIBLE.md)).

**Characters must be missable.** They are not always waiting. When a Character is running real work elsewhere, they are genuinely elsewhere — and the user can go find them.

**Absence must be true.** A Character is only unavailable when a real Instance is occupied. The World never stages fake busyness to seem alive.

**Users should learn habits.** Over months, a user should instinctively know where each Character usually is. Familiarity is the goal — the World should feel like home.

---

## Time

**Time is part of the interface.** Past, present and future are all visible: History (what happened), live activity (what is happening), queued Quests (what happens next).

**The world continues in your absence.** On return, Epoch narrates what actually occurred — real Activities, real Knowledge, real completions. If nothing ran while the user was away, the World says so honestly rather than inventing a story.

**Nothing is temporary.** Finished Eras do not disappear; they become part of the World's history.

---

## Composition and restraint

**Nothing is decorative.** Every object has architectural meaning. If a designer cannot name what a visual element represents in the engine, it does not ship.

**The World always has something interesting happening** — when there is something real to show. Interest comes from real activity and ambient life, never from fabricated events.

**Productivity lives inside the buildings.** Outside is a SNES-era JRPG world; inside each building is a serious, professional workspace. The transition should feel natural. The game is the navigation; the building is the tool.

**Immersion never costs productivity.** If the fantasy makes real work slower, the fantasy yields. A user in a hurry must always be able to get to the work.

---

## Visual language

**Living Pixel World, not pixel-art nostalgia.** The inspiration is the SNES era — especially *Chrono Trigger* and *Chrono Cross* — not to imitate them, but to capture the feeling of entering a handcrafted world full of life. No copied assets, characters or music; original work in that spirit.

**The World breathes.** Ambient motion, light, weather and small life continue at all times, at low amplitude, so the World never feels like a static screenshot.

**The World evolves.** It should visibly change over weeks and months as Eras progress and history accumulates. A returning user should see that time passed.

**The World feels inhabited.** Occupied spaces, traces of work, evidence that someone was here.

---

## Presentation vs Engine

The fantasy never leaks into the engine. The engine never limits the fantasy.

| Engine (canonical) | Presentation (World) |
|---|---|
| Project | Era |
| Conversation | Chronicle |
| Automation / Workflow | Quest |
| Team | Party |
| Timeline | History |
| Knowledge Engine | Library |
| Definition Registry | the Guild |
| Models & Integrations | the Research Lab |
| Active Project | Command Center |
| Automations | Automation Hub |
| Settings | the Settings hall |

Presentation remains a projection. Internal First remains intact (ADR-0003, ADR-0013).

---

## What we call it

Everything user-facing refers to Epoch simply as **the World**.

People should naturally say *"I'm going back to my World"* — never *"I'm opening Epoch."*

---

## Design review checklist

Before any user-facing feature ships, it must answer yes to all of these:

1. Does every visual element map to something real in the engine?
2. Does motion communicate information before text does?
3. Is any implied activity actually happening?
4. Can the user understand what is going on in under one minute?
5. Does this respect the Character Bible for any Character involved?
6. Does silence/stillness remain readable and honest?
7. Does immersion cost the user any real productivity? (If yes, fix it.)

---

## Content is replaceable
Every visual and audio element in this guide is resolved through the active World Pack - see [CONTENT_PHILOSOPHY.md](CONTENT_PHILOSOPHY.md) and [ADR-0016](docs/ADR/0016-asset-resolution.md). This guide governs **how** the World moves and communicates; the World Pack governs **what it looks and sounds like**. The rules here hold for every pack.


---

## The Engine owns reality; the UI owns animation

Presence is produced by the **World Simulation** in the Engine ([ADR-0018](docs/ADR/0018-world-simulation.md)), never by the client. The Engine publishes authoritative `PresenceState` (current place, destination, progress, speed, ETA, current activity) on meaningful transitions — `StartedTravelling`, `ProgressUpdated`, `Arrived`, `ChangedDestination`, `StartedWorking`, `BecameIdle`.

The UI **interpolates smoothly between two authoritative states**. That is the whole of its licence: animation never creates information.

Every presence transition traces to a real cause — an Activity, an Instance state change, or a declared routine rule. Routine-driven idle movement is legitimate (being available is true information) but **must be visually distinguishable from working movement**, or the World becomes unreadable.

The World is not simulated while the application is closed. "While you were away" is reconstruction from real Activities and Knowledge — never simulated backfill.