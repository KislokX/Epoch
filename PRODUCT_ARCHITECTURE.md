# Product Architecture

> A design constitution — **not** an ADR.
>
> It answers exactly one question: **where does a capability belong?**
>
> It does not describe how the Engine works (see `ARCHITECTURE.md` and the ADRs), how the
> World behaves (`LIVING_WORLD_DESIGN_GUIDE.md`), or how Epoch should feel
> (`EXPERIENCE_CONSTITUTION.md`).

---

## The core distinction

Epoch is not an interface. It is one system that people experience in several different ways.

An **Experience Surface** is an independent way of interacting with the same Engine. Each has
its own interaction model, visual language, pacing and responsibilities. None of them holds
business logic, none of them is a separate application, and none of them replaces another.

```
Domain Kernel      canonical types · zero I/O · depends on nothing
      ↑
Engine             all business logic · the source of truth
      ↑
Experience Surfaces        Launcher · World · CLI · Mobile · …
```

## The Kernel is not the Engine

This distinction is load-bearing and easy to blur, so it is stated plainly.

The **Domain Kernel** (`epoch-kernel`) holds canonical types and performs **no I/O**. It knows
nothing about providers, packs, storage or presentation. "The Kernel depends on nothing" is
enforced by the crate boundary rather than by discipline (ADR-0003).

The **Engine** (`epoch-engine`) owns everything that happens: providers (ADR-0007),
capabilities (ADR-0005), execution (ADR-0008), knowledge (ADR-0010), persistence (ADR-0014),
the Activity Stream (ADR-0015), the World Simulation (ADR-0018).

**Experience Surfaces sit over the Engine.** No surface ever touches the Kernel directly, and
the Engine never learns which surface is consuming it — that is what makes headless operation
true rather than aspirational.

Writing "the Kernel owns persistence" would eventually put I/O inside `epoch-kernel` and
destroy a boundary the compiler currently guarantees. It owns none of it.

## Canonical vocabulary and presentation vocabulary

The domain language never changes. Surfaces are free to project it.

| Domain (canonical) | Presentation (user-facing) |
|---|---|
| Workspace / Scope | **World** |
| Project | Era |
| Automation | Quest |
| Conversation | Chronicle |
| Team | Party |
| Timeline | History |

So when the Launcher asks *"which Worlds exist?"*, the Engine is answering about
**Workspaces** (ADR-0013). A **World** as the user experiences it is a Workspace plus the
World Pack that gives it a face (ADR-0017).

The poetry never leaks inward. Internal First stays intact.

## The surfaces

### Launcher — preparation

Administration, optimised for clarity rather than immersion. It exists before a World.

Which Workspaces exist · which providers are connected · which models are installed · which
characters are available · which assets and World Packs are installed · settings.

The Launcher manages the system. It does not try to become the World.

### World — immersion

Where people experience Epoch. Agents live there, Places exist there, work happens there,
time passes there. Users are not opening software; they are returning to an ecosystem.

Who is working · what is happening · where should I go · what is this Place for · what changed
while I was away · how is my ecosystem evolving.

The World demonstrates the system. It does not administer it.

The World is the only surface with an **Experience Layer** (ADR-0022) — camera, focus, reveal
and atmosphere exist because immersion is its responsibility. The Launcher and the CLI need
none of that, and giving them one would be complexity with nothing to justify it.

### CLI — automation

Commands in, text out. Scripting, integration, unattended execution.

### Further surfaces

Mobile, VR, a read-only dashboard, a remote console. Each must answer a user need the others
do not. **None may require a change to the Engine** — that is the test of whether this
abstraction is real.

## Architecture is not roadmap

Every surface is architecturally equal. Which ones exist today is a roadmap question and
lives in `ROADMAP.md`, never here — otherwise the architecture would have to be rewritten
every time something ships.

## Surfaces never duplicate behaviour

A capability may appear on several surfaces. The behaviour stays in the Engine; each surface
expresses it in the way that matches its responsibility.

Connecting a provider is administration, so the Launcher owns configuring it — and the World
still shows the consequence, because a configured provider is what brings the Laboratory to
life (ADR-0021). Same Engine fact, two honest expressions, no duplicated logic.

## The routing question

Before building anything, answer two questions in order:

1. **Which Experience Surface owns this capability?** — determines *where* it belongs.
2. **How should that surface express it?** — determines *how* people experience it.

A useful tie-breaker for the second: **can the World communicate this by observation?** If
yes, prefer the World. If it needs precision, configuration or administration, it is the
Launcher's.

| Capability | Surface |
|---|---|
| Connect a provider | Launcher |
| Create a Workspace | Launcher |
| Configure a character | Launcher |
| See that the Researcher is working, and where | World |
| Visit the Laboratory | World |
| Run a batch unattended | CLI |

## What this does not mean

*"Features belong to Places"* (`EXPERIENCE_CONSTITUTION.md`) is a preference, not a law.
Administration that would be worse as a building belongs in the Launcher. The routing question
decides; "whenever possible" does not mean "always".

---

The Engine guarantees truth. Surfaces express it. Each optimises for a different kind of
interaction, and together they are one product rather than several applications.
