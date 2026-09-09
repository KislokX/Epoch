# Epoch (AIOS)

A living world powered by AI. Not an AI orchestrator - a place where intelligence has
continuity: Characters work, knowledge persists, projects become Eras in the world's history,
and time has meaning.

> "I entered my world" - never "I opened software."

Epoch is not a single interface. It is one Engine experienced through several **Experience
Surfaces**: the Launcher prepares, the World immerses, the CLI automates. The user joins the
world; they do not start it.

North star: **AI should never be harder to use than a video game.**

- Simplicity beats cleverness. Great defaults beat configuration.
- Explainable, not magical. Understandable autonomy over hidden behaviour.
- Local-first when possible. Provider-agnostic always.
- Detect what is already installed; connect it in a few clicks. From install to productive in
  under five minutes.

This repository contains the implementation (`BUILD/`) and the Obsidian knowledge base
(the project's second brain).

## The six pillars

Each answers exactly one architectural question. If two documents ever answer the same
question, they should become one.

| Pillar | Document | Question it answers |
|---|---|---|
| **Skeleton** | [docs/ADR/_index.md](docs/ADR/_index.md) | How the system works |
| **Map** | [PRODUCT_ARCHITECTURE.md](PRODUCT_ARCHITECTURE.md) | Where a capability belongs |
| **Soul** | [EXPERIENCE_CONSTITUTION.md](EXPERIENCE_CONSTITUTION.md) | How Epoch should feel |
| **Body** | [LIVING_WORLD_DESIGN_GUIDE.md](LIVING_WORLD_DESIGN_GUIDE.md) | How the World behaves and communicates |
| **Heart** | [CHARACTER_BIBLE.md](CHARACTER_BIBLE.md) | Who lives inside the World |
| **Skin** | [CONTENT_PHILOSOPHY.md](CONTENT_PHILOSOPHY.md) | What may ship, and who owns the art |

## Also start here

- **How we work:** [CLAUDE.md](CLAUDE.md) - the operating constitution
- **How Epoch is built:** [ARCHITECTURE.md](ARCHITECTURE.md) + [docs/Architecture/_index.md](docs/Architecture/_index.md)
- **How we decide:** [docs/Architecture/Principles & Runway.md](docs/Architecture/Principles%20&%20Runway.md)
- **How we build:** [docs/Build/Build From Life.md](docs/Build/Build%20From%20Life.md)
- **What is next:** [ROADMAP.md](ROADMAP.md) + [docs/Milestones](docs/Milestones)
- **The World (UX hub):** [docs/UX/World.md](docs/UX/World.md)
- **Where we came from:** [docs/History](docs/History) - superseded documents, kept for provenance

## The layers, in one diagram

```
Domain Kernel        canonical types, zero I/O, depends on nothing
      |
Engine               all business logic, the source of truth
      |
Experience Surfaces  Launcher . World . CLI . Mobile . ...
      |
(World only) Experience Layer -> Renderer -> Assets
```

The Engine answers *what is true*. The Experience Layer answers *how that truth is
experienced*. The Renderer answers *how that experience is drawn*.
