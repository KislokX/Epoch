# Architecture

> **How is Epoch built?**
>
> Where a capability *belongs*: `PRODUCT_ARCHITECTURE.md`. Why each decision was made:
> [docs/ADR/_index](docs/ADR/_index.md). Subsystem detail:
> [docs/Architecture/_index](docs/Architecture/_index.md).

## Shape

All business logic lives in the Rust **Engine**. Presentation is a thin, replaceable
projection over IPC commands and event subscriptions. The Engine runs **headless**, which is
what makes several Experience Surfaces possible over one system rather than several
applications (ADR-0003).

```
Experience Surfaces      Launcher . World . CLI . Mobile . ...
        |                (World only: Experience Layer -> Renderer -> Assets)
        |  IPC: commands + event subscriptions
--------+---------------------------------------------------------------
        v                       ENGINE (Rust)
Application     Projects, Context Composer, Cost, Orchestration
Domain          Definition Runtime, Definition Registry, World Simulation
Kernel          canonical types, ZERO I/O                <- depends on nothing
Ports           Providers . Tools . Integrations         <- trichotomy (ADR-0004)
Infrastructure  Persistence, Activity Stream, Config/Secrets, FS/Terminal/Git
```

The Kernel is not the Engine. `epoch-kernel` holds canonical types and performs no I/O; its
independence is enforced by the crate boundary rather than by discipline. Everything that
*happens* is the Engine's.

## Crates

| Crate | Responsibility |
|---|---|
| `epoch-kernel` | Canonical vocabulary and types. Zero I/O. Depends on nothing |
| `epoch-engine` | All business logic: World Packs, Places, assets, definitions, simulation, projection |
| `epoch-tauri` | The desktop shell. Owns no logic — starts the Engine, exposes IPC, forwards projections |
| `ui/` | Presentation. Consumes contracts, never the Engine's internals |

Everything leaving the Engine is a **projection** (Internal First). The presentation layer
declares those contracts by hand in `ui/src/ipc/contracts.ts`, and a test in the Engine
(`tests/wire_contract.rs`) is the seam that fails when the two drift.

## Stack

Rust · Tauri · React · TypeScript · SQLite.

Chosen in ADR-0001 (desktop shell) and ADR-0002 (core language). Dependencies are kept to a
minimum, and each one must justify why it is needed and whether it is foundational or
replaceable.

## Foundational decisions

- **0001** Tauri shell · **0002** Rust core · **0003** Engine / Presentation separation
- **0004** Provider / Tool / Integration trichotomy (MCP is one integration, not the base)
- **0005** Capability-first architecture — the Engine reasons in capabilities, not names
- **0006** Conversation Domain Model — providers are translators
- **0011** Runtime over Configuration · **0012** Context is composed, never concatenated
- **0015** Commands cause; Activities record
- **0016**–**0017** Asset resolution, archetypes and World Packs
- **0018** World Simulation — the Engine owns reality, the UI owns animation
- **0019**–**0021** Projection pipeline, assets, Places as compositions
- **0022** Visit and the Experience Layer

Full ledger with horizons: [docs/ADR/_index](docs/ADR/_index.md).

## Doctrine

The architecture was frozen on 2026-07-25 after a readiness review. Since then, **every
architectural change must cite implementation evidence** — what the build actually revealed,
never what we can imagine. ADRs 0019 to 0022 each name the code that forced them.

Implementation prioritises complete vertical slices over horizontal subsystem completion, and
every milestone must make the World visibly more alive
([docs/Build/Build From Life](docs/Build/Build%20From%20Life.md)).
