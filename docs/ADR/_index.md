# ADR Index

> **FOUNDATION COMPLETE 2026-07-25** - ADRs 0001-0018 accepted, architecture frozen ([[../Architecture/Readiness Review]]).
> New ADRs and amendments must cite implementation evidence.
>
> ADRs 0019-0023 were all written *after* the freeze and each cites what the build revealed:
> 0019 a renderer switching on place concepts, 0020 inline geometry no real tool could author,
> 0021 shadow/spawn/label constants living in React, 0022 a viewport welded to the world's
> extent and a camera stored instead of derived, 0023 a Launcher reporting a population that
> did not exist because archetype was doing the work of an identity, 0026 a per-character
> parameter set whose first draft carried an Ollama parameter name into the Kernel.

Architecture Decision Records for AIOS (AI Operating System).

An ADR captures **one** significant, hard-to-reverse decision. Status flow:
`Proposed -> Accepted -> (Superseded by ADR-XXXX | Deprecated)`.
Every ADR declares a **Horizon**: IMPLEMENT NOW / DESIGN NOW / VISION (see [[../Architecture/Principles & Runway]]).

## Ledger

| ADR                                                  | Title                                                  | Status                     | Horizon       | Depends on                   |
| ---------------------------------------------------- | ------------------------------------------------------ | -------------------------- | ------------- | ---------------------------- |
| [0001](0001-tauri.md)                                | Desktop shell: Tauri                                   | Accepted                   | IMPLEMENT NOW | -                            |
| [0002](0002-rust.md)                                 | Core language: Rust                                    | Accepted                   | IMPLEMENT NOW | -                            |
| [0003](0003-engine-presentation-separation.md)       | Engine / Presentation separation                       | Accepted                   | IMPLEMENT NOW | 0001, 0002                   |
| [0004](0004-provider-tool-integration-trichotomy.md) | Provider / Tool / Integration trichotomy               | Accepted                   | DESIGN NOW    | 0003                         |
| [0005](0005-capability-first-architecture.md)        | Capability-first architecture (Profiles)               | Accepted                   | IMPLEMENT NOW | 0003                         |
| [0006](0006-conversation-domain-model.md)            | AIOS Conversation Domain Model                         | Accepted                   | IMPLEMENT NOW | 0003                         |
| [0007](0007-provider-abstraction.md)                 | Provider Abstraction & Registry                        | Accepted (amended by 0027; amended 2026-08-02) | IMPLEMENT NOW | 0004, 0005, 0006             |
| [0008](0008-execution-engine.md)                     | Executable Capability & Execution Engine               | Accepted (amended by 0015; amended 2026-08-02) | IMPLEMENT NOW | 0004, 0005                   |
| [0009](0009-trust-engine.md)                         | Trust Engine & Autonomy Modes                          | Accepted (amended 2026-07-31 ×2, by 0027) | mixed | 0008, 0005                   |
| [0010](0010-knowledge-engine.md)                     | Knowledge Objects & Knowledge Engine                   | Accepted (amended by 0015) | mixed         | 0006, 0008                   |
| [0011](0011-definition-runtime.md)                   | Definition-Driven Runtime (Runtime over Configuration) | Accepted (amended by 0015) | mixed         | 0003, 0005, 0008             |
| [0012](0012-context-composer.md)                     | Context Composer (composed, never concatenated)        | Accepted (amended by 0015; twice from building it) | mixed | 0005, 0006, 0010, 0011 |
| [0013](0013-scope-model.md)                          | Scope Model (Application/Workspace/Project/Session)    | Accepted                   | mixed         | 0011, 0010                   |
| [0014](0014-persistence-contract.md)                 | Persistence Contract & Workspace Portability           | Accepted                   | mixed         | 0013, 0006, 0010, 0011       |
| [0015](0015-activity-stream.md)                      | Activity Stream (Commands cause, Activities record)    | Accepted                   | mixed         | 0003, 0008, 0010, 0014       |
| [0016](0016-asset-resolution.md)                     | Asset Resolution & World Packs                         | Accepted (amended by 0017, 0023) | mixed   | 0003, 0005, 0011, 0013       |
| [0017](0017-character-archetypes-world-packs.md)     | Character Archetypes & World Packs                     | Accepted (amended by 0023, 0028) | mixed         | 0011, 0013, 0016             |
| [0018](0018-world-simulation.md)                     | World Simulation (Presence)                            | Accepted                   | mixed         | 0003, 0011, 0014, 0015, 0017 |
| [0019](0019-projection-pipeline.md)                  | Projection Pipeline (Renderers)                        | Accepted (amended by 0021) | mixed         | 0003, 0016, 0017             |
| [0020](0020-asset-pipeline.md)                       | Asset Pipeline & Renderables                           | Accepted (amended by 0021) | mixed         | 0016, 0017, 0019             |
| [0021](0021-places-as-compositions.md)               | Places are Compositions                                | Accepted (amended by 0022, 0028) | mixed         | 0016, 0017, 0019, 0020       |
| [0022](0022-visit-and-the-experience-layer.md)       | Visit and the Experience Layer                         | Accepted                   | mixed         | 0003, 0018, 0021             |
| [0023](0023-characters-belong-to-the-user.md)        | Characters belong to the user                          | Accepted                   | mixed         | 0011, 0016, 0018             |
| [0024](0024-imported-artwork.md)                     | Imported artwork, and the door it comes through        | Accepted                   | mixed         | 0016, 0020, 0023             |
| [0025](0025-quests-own-the-work.md)                  | Quests own the work                                    | Accepted                   | mixed         | 0006, 0008, 0009, 0011, 0012 |
| [0026](0026-characters-are-portable-assets.md)       | Characters are portable assets; Providers declare their own surface | Accepted (amended 2026-08-02) | mixed | 0005, 0007, 0011, 0014, 0016, 0023 |
| [0027](0027-brains-models-and-agents.md)             | Brains — a character thinks with a model or works with an agent | Accepted (amended 2026-08-02) | mixed | 0003, 0005, 0007, 0008, 0009, 0025, 0026 |
| [0028](0028-the-map-belongs-to-the-user.md)           | The map belongs to the user                            | Accepted | mixed | 0016, 0021, 0023, 0024 |
| [0029](0029-bridges-and-the-worlds-machines.md)      | Bridges — a World spans machines, and one owns reality  | Accepted | mixed | 0003, 0007, 0009, 0012, 0026, 0027 |
| [0030](0030-images-are-made-by-a-capability.md)      | Images are made by a capability, not by a brain (+Styles, +derived Styles, +refusals) | Accepted | mixed | 0007, 0009, 0016, 0025, 0026 |
| [0031](0031-the-asset-registry.md)                   | The Asset Registry — one Workshop, many catalogues (+three shelves) | Accepted | mixed | 0003, 0007, 0009, 0016, 0024, 0030 |
| [0032](0032-the-generative-library.md)               | The Generative Library — Epoch owns it, the engine reads it | Accepted | mixed | 0016, 0024, 0029, 0030, 0031 |
| [0033](0033-the-studio-panel.md)                     | The Studio Panel — the character opens it, the user fills it in | Accepted | mixed | 0005, 0009, 0025, 0026, 0030, 0031, 0032 |
| [0034](0034-work-that-outlasts-the-turn.md)          | Work that outlasts the turn | Accepted | mixed | 0005, 0009, 0015, 0018, 0025, 0030 |

## Pending (surfaced, not yet designed)

- Tool capabilities: the **contract** is built (`epoch-kernel::capability`, `epoch-engine::capability`;
  ADR-0009 amended 2026-07-31 — risk is derived, not declared). The Phase-1 **set** of actual
  capabilities is still open, and waits on the Trust Engine - NEXT.
- World concept vocabulary curation (before first sprite). Product references show six places
  (bridge, library, laboratory, forge, communications, observatory); the Engine knows five
  concepts. Communications and Observatory map to capabilities described in
  docs/Features/Chat.md and docs/Features/Timeline.md. The vocabulary grows additively when
  those capabilities are built (ADR-0017), never to fill a mockup.
- Place identity for Places that project no capability - currently impossible by construction
  (ADR-0022); revisit only if a real need appears.
- Official Epoch Universe - deferred creative milestone (docs/Milestones/Official Epoch Universe.md).
- Integration Layer (auto-discovery).
- Config/Secrets, Project System, Cost, Orchestration/Automation, Memory store, UI Shell.

## ADR template

```markdown
# ADR-XXXX: <title>
- Status: Proposed | Accepted | Superseded | Deprecated
- Date: YYYY-MM-DD
- Depends on: <ADR links>
- Related architecture: <doc links>
- Horizon: IMPLEMENT NOW | DESIGN NOW | VISION | mixed
## Context
## Decision
## Why this is best
## Alternatives considered
## Consequences
## Assumptions
## Risks
## Open questions
```

