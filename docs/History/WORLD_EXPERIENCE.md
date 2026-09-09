> **ARCHIVED 2026-07-27. Superseded by EXPERIENCE_CONSTITUTION.md (the experience principles) and ADR-0022 (the enforceable Visit contract). Kept for provenance: this document first separated Reveal from graphical level of detail.**

---

# WORLD_EXPERIENCE.md

# World Experience

> *"Users do not navigate software.
> They return to a living world."*

---

# Purpose

This document defines **how users progressively experience the World**.

It does **not** redefine the Engine.

It does **not** redefine the Living World.

It does **not** redefine the Experience Manifesto.

Instead, it defines the experiential layer that sits above them.

The Engine defines reality.

The World visualizes reality.

The Experience Layer orchestrates how reality is progressively experienced.

---

# Core Principle

The World progressively reveals itself.

The World does not become another application.

The World does not switch interfaces.

The World does not replace itself.

The user simply experiences more of the same World.

---

# World Experience Contract

Every interaction inside a World follows the same progression.

```
Place
    ↓
Visit
    ↓
Reveal
    ↓
Place View
    ↓
Place Experience
```

The Place never changes.

Only the amount of experience available changes.

---

# Visit

Visit is a stable interaction contract.

Users should only learn one interaction:

```
Visit Place
```

What "Visit" means may evolve forever.

The interaction itself never changes.

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

Camera is not the experience.

Camera simply begins the experience.

The Experience Layer orchestrates everything that follows.

---

# Experience Evolves

Interaction remains stable.

Experience grows.

```
Visit
        ↓
Experience v1
        ↓
Experience v2
        ↓
Experience v3
        ↓
Experience vN
```

Users never need to relearn interaction.

Only the richness of the World increases.

---

# The Experience Layer

The Experience Layer is not another source of truth.

It derives everything from:

- Engine State
- User Intent
- Time

It never invents facts.

It only expresses existing reality.

The Experience Layer may orchestrate:

- camera
- focus
- reveal
- transitions
- ambient audio
- particle systems
- emphasis
- contextual presentation

It may never create functionality that does not exist.

---

# Reveal

Reveal is not distance.

Reveal is not graphical detail.

Reveal is the amount of a Place that is currently being experienced.

Reveal is always constrained by reality.

The Experience Layer may never reveal capabilities that do not exist.

The World should always remain truthful.

---

# Place Views

A Place has one identity.

Never multiple identities.

A Place may progressively reveal different views of itself.

```
World View
        ↓
Place View
        ↓
Place Experience
```

These are not different Places.

They are different ways of experiencing the same Place.

No new application is opened.

No modal appears.

No secondary scene is loaded.

The World simply allows the user to come closer.

---

# Sense of Place

Every Place should communicate its identity.

Not only visually.

Emotionally.

Atmospherically.

Every Place should feel different before the user even begins interacting.

Examples:

## Research Laboratory

- subtle machinery
- electrical ambience
- quiet focus
- research particles

## Library

- warm lighting
- page turning
- silence
- calm atmosphere

## Forge

- sparks
- rhythmic hammering
- fire
- metal resonance

Atmosphere communicates identity.

Atmosphere must never fake activity.

Idle should never resemble work.

---

# World View

The World is always the user's starting point.

The World is already discovered.

The camera simply establishes where the user currently is.

Exploration should come from curiosity.

Never from hiding information.

The minimap exists to communicate that the World extends beyond the current viewport.

The camera moves.

The World does not.

---

# Progressive Experience

The same Visit interaction may become richer over time.

Today:

```
Visit
    ↓
Camera
    ↓
Subtitle
    ↓
Occupants
    ↓
Particles
    ↓
Ambient Audio
```

Tomorrow:

```
Visit
    ↓
Camera
    ↓
Reveal
    ↓
NPC Greeting
    ↓
Ambient Transition
    ↓
Interactive Consoles
    ↓
Additional Authored Assets
    ↓
Richer Interaction
```

The interaction never changes.

Only the experience grows.

---

# World First

Whenever something can naturally be communicated through the World...

Prefer the World.

Whenever precision, configuration or administration are required...

Use the UI.

The UI frames the World.

The World remains the primary user experience.

---

# Asset Pipeline

The Engine never generates artwork.

Whenever implementation requires new visual assets:

```
Implementation
        ↓
Missing Assets
        ↓
Asset Request
        ↓
Artwork Production
        ↓
Repository
        ↓
Integration
```

Creative direction happens outside the Engine.

The Engine consumes authored content.

Implementation should pause whenever authored assets are required.

Placeholder artwork should never become production architecture.

---

# Responsibilities

The responsibilities of each layer are intentionally separated.

## Engine

Defines reality.

## World

Visualizes reality.

## Experience Layer

Orchestrates how reality is experienced.

## Renderer

Draws frames.

## Assets

Define visual identity.

---

# Design Question

Before proposing any interaction, always ask:

> **"What should the user experience during this moment?"**

Only afterwards ask:

> **"How should this be implemented?"**

Experience should always drive implementation.

Never the opposite.