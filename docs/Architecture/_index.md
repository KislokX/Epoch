# Architecture Index - AIOS Engine

> **FOUNDATION COMPLETE - architecture frozen 2026-07-25.** See [[Readiness Review]].
> Changes now require **implementation evidence**, not speculation. Build Phase: [[../Milestones/Walking Skeleton]].

> North star: **AI should never be harder to use than a video game.**
> Standing principles: [[Principles & Runway]] (Internal First, Runtime over Configuration, Context is composed, Commands cause/Activities record, Engine owns reality/UI owns animation, Snapshot preserves continuity, Earn Complexity, Architecture Runway).

All business logic lives in the Rust **AIOS Engine** ([[../ADR/0003-engine-presentation-separation]]). React is presentation only, over IPC commands + events. The engine runs headless.

## Scope hierarchy ([[Scope Model]])
Application > Workspace > Project > Session. Workspace = portable unit; Definitions live at Workspace, referenced by Projects.

## Layered model
```
UI (React, presentation only)
        |  IPC: commands + event subscriptions
--------+-------------------------------------------
        v        AIOS ENGINE (Rust)
Application   (Projects, Context Composer, Cost, Orchestration, Execution, Trust, Knowledge, Observability)
Domain        (Definition Runtime + Instances, Definition Registry, Scope Model)
Kernel        (Conversation + Capability + Knowledge + Definition + Context + Scope + Activity)  <- depends on nothing
Ports         Providers . Tools . Integrations   (all become Executable Capabilities)
Infrastructure (Persistence [contract+adapter], Activity Stream, Config/Secrets, FS/Terminal/Git)
```

## Interaction model
- **Commands** (synchronous, need a response) drive the turn loop.
- **Activities** (asynchronous, immutable) flow through the [[Activity Stream]]; consumers: UI, Knowledge Engine, Automation, Observability, Plugins. Persistence never subscribes; repositories = source of truth.

## The turn loop (Commands) + observation (Activities)
resolve model -> Context Composer -> Provider call -> stream -> ToolCalls -> Execution (Trust-gated). Alongside: each step emits Activities; Knowledge Engine derives KnowledgeObjects from them.

## Subsystem map

| # | Subsystem | Layer | Horizon | Status | Doc |
|---|-----------|-------|---------|--------|-----|
| 0 | Domain Kernel | Kernel | IMPLEMENT NOW | Designing | [[Domain Kernel]] |
| 0b | Capability System | Kernel (cross-cut) | IMPLEMENT NOW | Designing | [[Capability System]] |
| 1 | Provider Abstraction | Ports | IMPLEMENT NOW | Designed | [[Provider Layer]] |
| 2 | Execution Engine | Application | IMPLEMENT NOW | Designed | [[Execution Engine]] |
| 3 | Trust Engine & Autonomy | Application (cross-cut) | mixed | Designed | [[Trust Engine]] |
| 4 | Knowledge Engine | Application | mixed | Designed | [[Knowledge Engine]] |
| 5 | Definition Runtime (+ Instances) | Domain | mixed | Designed | [[Definition Runtime]] |
| 6 | Definition Registry | Domain | IMPLEMENT NOW | Designed | [[Definition Registry]] |
| 7 | Context Composer | Application | mixed | Designed | [[Context Composer]] |
| 8 | Scope Model | Domain | mixed | Designed | [[Scope Model]] |
| 9 | Persistence (contract + adapter) | Infra | mixed | Designed | [[Persistence]] |
| 10 | Activity Stream | Infra | IMPLEMENT NOW | Designed | [[Activity Stream]] |
| 11 | Observability (+ Activity Recorder) | Application | mixed | Designed | [[Observability]] |
| 12 | World Packs (narrative + asset resolution) | Presentation-support | mixed | Designed | [[World Packs]] |
| 13 | World Simulation (Presence) | Application | mixed | Designed | [[World Simulation]] |
| 14 | Tool Interface | Ports (a capability kind) | DESIGN NOW | Pending | - |
| 13 | Integration Layer | Ports | DESIGN NOW | Pending | - |
| 14 | Orchestration / Automation | Application | DESIGN NOW | Pending | [[Automation Engine]] |
| 15 | Memory store | Application | DESIGN NOW | Pending | [[Memory]] |
| 16 | Project System | Application | IMPLEMENT NOW | Pending | [[../Features/Projects]] |
| 17 | Cost & Telemetry | Application (cross-cut) | DESIGN NOW | Pending | - |
| 18 | Config & Secrets | Infra | IMPLEMENT NOW | Pending | - |
| - | UI Shell | Presentation | IMPLEMENT NOW | Pending | [[../UX/Design System]] |

## Design order (spine first)
Provider -> Execution -> Trust -> Knowledge -> Definition Runtime + Registry -> Context Composer -> Scope + Persistence -> **Activity Stream + Observability** -> Tool -> Integration -> Orchestration -> Memory -> Project -> Config -> UI.

## ADR ledger
See [[../ADR/_index]]. Accepted: 0001-0018. **The foundational spine is complete** - remaining subsystems are consumers.

