> **ARCHIVED 2026-07-27. Superseded by EXPERIENCE_CONSTITUTION.md and PRODUCT_ARCHITECTURE.md. Kept for provenance: this document first proposed that the Launcher and the World are two experiences rather than one interface, which became the Experience Surfaces abstraction.**

---

# EXPERIENCE_PHILOSOPHY.md

# Epoch Experience Philosophy

> *"The Launcher prepares the journey.
> The World is the journey."*

---

# Purpose

This document defines the product experience of Epoch.

It does not redefine the Engine.

It does not redefine the Living World.

It does not redefine the Experience Manifesto.

It defines how users experience Epoch as a complete product.

The Launcher prepares.

The World immerses.

Together, they create the complete Epoch experience.

---

# Two Experiences

Epoch is composed of two distinct experiences.

Not one.

Trying to merge them into a single interface creates unnecessary compromises.

Keeping them separate allows each to excel at its purpose.

```
Launcher
        ↓
Board Ship
        ↓
Travel
        ↓
Arrive
        ↓
World
        ↓
Visit
        ↓
Reveal
        ↓
Experience
        ↓
Work
        ↓
Leave World
        ↓
Launcher
```

The transition between them should always feel intentional.

---

# Experience 1 — The Launcher

The Launcher exists to prepare the expedition.

It is not immersive.

It is efficient.

Its responsibility is administration.

Typical responsibilities include:

- Worlds
- Connections
- Characters
- Workshop
- Settings
- Updates
- Assets
- Providers
- Plugins
- Models

The Launcher prepares.

It does not attempt to become the World.

---

# Experience 2 — The Living World

The World begins when the user boards a World.

The World is not another application.

The World is a place.

It exists independently from the user.

Agents live there.

Places exist there.

Work happens there.

Time passes there.

The user is not opening software.

The user is returning to an ecosystem.

---

# Crossing the Boundary

Entering a World should feel like crossing a boundary.

Not opening another screen.

```
Launcher

↓

Board Ship

↓

Travel

↓

Arrival

↓

World
```

The Launcher ends.

The World begins.

Every design decision after this point should prioritize immersion before efficiency.

---

# The World Progressively Reveals Itself

The World does not replace itself.

The World does not switch applications.

The World does not open interfaces.

Instead, the World progressively reveals more of itself.

```
World

↓

Visit

↓

Reveal

↓

Place View

↓

Place Experience
```

The user remains inside the same World throughout the experience.

---

# The World Is the Primary Experience

The World is more than an interface.

It is the primary way users experience Epoch.

Traditional interface elements still exist.

However, they exist to support the World.

Never to compete with it.

The UI frames the World.

The World remains the center.

---

# Visit Is a Contract

Users should only learn one interaction.

```
Visit Place
```

What "Visit" means evolves.

The interaction never changes.

```
Visit

↓

Camera

↓

Focus

↓

Reveal

↓

Experience
```

Tomorrow the same Visit may include:

- ambient transitions
- authored animations
- NPC greetings
- additional interactions
- richer presentation

The contract remains stable.

---

# Experience Before Implementation

Every proposal should begin with experience.

Never implementation.

Before asking:

> "How should this work?"

Ask:

> "What should the user feel during this moment?"

Experience drives architecture.

Architecture enables implementation.

Implementation delivers experience.

---

# World First

Whenever something can naturally be communicated through the World...

Prefer the World.

Whenever precision, configuration or administration are required...

Use the UI.

Examples:

The World communicates:

- presence
- movement
- collaboration
- atmosphere
- identity
- exploration
- discovery

The UI communicates:

- configuration
- administration
- statistics
- logs
- settings
- notifications
- management

Neither replaces the other.

They complement each other.

---

# Experience Layer

The Experience Layer orchestrates presentation.

It is not another source of truth.

It derives everything from:

- Engine State
- User Intent
- Time

It never invents facts.

It only determines how reality is experienced.

Examples include:

- camera movement
- focus
- reveal
- transitions
- ambient audio
- particles
- emphasis
- contextual presentation

Reality always comes from the Engine.

---

# Sense of Place

Every Place should have an identity.

Not only visually.

Emotionally.

Atmospherically.

The user should recognize a Place before interacting with it.

Atmosphere communicates purpose.

Atmosphere never invents activity.

Idle should never resemble work.

---

# Progressive Growth

Epoch should grow naturally over time.

A Place may progressively reveal:

- additional authored assets
- richer interactions
- new NPCs
- more visual storytelling
- more contextual information

The Place never becomes another Place.

The interaction never changes.

Only the richness of the experience increases.

---

# Asset Workflow

Artwork is authored.

Never generated by the Engine.

Whenever implementation requires new visuals:

```
Implementation

↓

Missing Assets

↓

Asset Request

↓

Artwork

↓

Repository

↓

Integration
```

The Engine consumes authored assets.

Creative direction happens outside the Engine.

Implementation should pause whenever authored assets are required.

---

# Design Questions

Before proposing a feature, ask:

**Can the World communicate this naturally?**

If yes...

Prefer the World.

If no...

Use the UI.

---

# Final Principle

Users do not navigate software.

They return to a living world.

The Launcher prepares.

The World immerses.

The Experience Layer orchestrates.

The Engine guarantees truth.

Together, they create the Epoch experience.

# The Player's Mental Model

Users should never think:

"I'm opening another tool."

Users should think:

"I'm going somewhere."

The World should always preserve spatial continuity.

Every Place exists somewhere.

Every Agent exists somewhere.

Every activity happens somewhere.

Moving through Epoch should feel like navigating a living ecosystem rather than switching between software features.