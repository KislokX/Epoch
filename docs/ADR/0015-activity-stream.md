# ADR-0015: Activity Stream (Commands cause, Activities record)

- Status: Accepted
- Date: 2026-07-23
- Depends on: [[0003-engine-presentation-separation]], [[0008-execution-engine]], [[0010-knowledge-engine]], [[0014-persistence-contract]]
- Amends: [[0008-execution-engine]], [[0010-knowledge-engine]] (knowledge is now derived from the stream, not emitted per-subsystem)
- Related architecture: [[Activity Stream]], [[Observability]], [[Knowledge Engine]]
- Horizon: in-memory pub/sub + emission + subscription + metadata **IMPLEMENT NOW**; Activity Recorder interface **DESIGN NOW**; replay / event-sourcing **VISION**

## Context
Subsystems were calling each other directly, and each execution was responsible for emitting a KnowledgeObject (ADR-0008/0010). That couples subsystems and spreads documentation duty everywhere. We want a nervous system for observability, knowledge derivation, automation and extensibility - without becoming an event-sourced source of truth.

## Decision
Elevate the Event Bus into the **AIOS Activity Stream**: a first-class, in-memory pub/sub of **immutable Activities**. Two interaction kinds:

- **Commands** - synchronous; caller needs a response. The turn loop stays Commands (Context Composer -> Provider; Instance -> Execution Engine).
- **Activities** - asynchronous, fire-and-forget; "something happened." Emitted alongside Commands.

Examples: AgentStarted, ContextComposed, ProviderRequestStarted/Completed, ToolExecutionStarted/Completed, KnowledgeCreated, ConversationUpdated, SnapshotCreated, TrustDecisionMade.

**Activities are transient observations; Knowledge is persistent understanding.** The **Knowledge Engine subscribes to the stream** and decides what becomes a persistent KnowledgeObject. This amends 0008/0010: subsystems emit Activities, not KnowledgeObjects.

**Hard rule:** no subsystem may depend on an Activity existing after emission. Activities are a delivery mechanism; **repositories remain the source of truth** (ADR-0014). **Persistence never subscribes to the stream.**

**Activity metadata (Kernel):** id, type, timestamp, scope ids (Session/Instance/Project), correlation/causation id, **Visibility** (Public/Internal), **Importance** (Debug/Normal/Critical), immutable payload. Metadata helps consumers filter - not for persistence.

**Consumers:** UI, Knowledge Engine, Automation Engine, Observability, Plugins, Debug Tools.

**Activity Recorder** (see [[Observability]]) is a *future* consumer that may persist append-only logs for replay/audit/diagnostics. Interface designed now; **no implementation in Phase 1**.

## Why this is best
- Decouples subsystems: emit, don't call, wherever a response isn't needed.
- Knowledge becomes a single consumer, not a per-subsystem duty.
- Observability/automation/plugins attach without touching the core.
- Stays cheap (in-memory) and honors Earn Complexity - no event-sourcing.

## Alternatives considered
- **Durable Activity log from Phase 1** - storage cost + a second data source; pushes toward event-sourcing early. Deferred to the Activity Recorder (DESIGN NOW).
- **Full event-sourcing (stream = source of truth)** - conflicts with ADR-0014; cathedral for Phase 1. Rejected.
- **Keep per-subsystem KnowledgeObject emission** - couples documentation everywhere. Rejected (amended).

## Consequences
- Activity is a Kernel type; the turn loop gains an observability layer without changing its Command path.
- Execution/Trust/Context emit Activities (e.g. ToolExecutionCompleted, TrustDecisionMade, ContextComposed); Knowledge Engine derives KnowledgeObjects from them.
- Transport is swappable (memory now; anything later) because nothing depends on Activity durability.

## Assumptions
- In-memory delivery is sufficient for Phase 1 consumers.
- Correlation ids make the stream reconstructable enough for live observability.

## Risks
- Lost Activities (in-memory, no delivery guarantee). *Mitigation:* by rule, nothing depends on them; durable needs go through the future Activity Recorder.
- Stream becoming a covert source of truth. *Mitigation:* explicit rule + Persistence never subscribes.

## Open questions
- Delivery semantics (sync-dispatch vs. queued) for Phase 1.
- Activity Recorder log format + retention (DESIGN NOW).

---

## Clarification (applied 2026-07-25): no self-derivation
A consumer must never derive from Activities it emitted itself. Concretely: the Knowledge Engine emits `KnowledgeCreated` with **Visibility: Internal** and ignores its own emissions, so knowledge derivation can never recurse. This uses existing Activity metadata; it adds no new concept.

General rule: **derivation flows one way.** If a subsystem both consumes and emits on the stream, it must exclude its own emissions from its input filter.
---

## Clarification (applied 2026-07-25): token streaming is NOT an Activity
Token-level streaming does **not** flow through the Activity Stream. It uses the **`StreamingEvent` channel that already exists** in the Domain Kernel (ADR-0006) and is returned by `Provider::stream` (ADR-0007): Provider -> Character Instance -> UI.

The Activity Stream carries **coarse, turn-level facts** (`ProviderRequestStarted` / `ProviderRequestCompleted`), never per-token events. `TokenStreamed` is removed from the Activity vocabulary.

Rationale: streaming is the **response to a Command**, not an asynchronous observation - "Commands cause; Activities record". Fanning hundreds of per-token events per turn to every subscriber (UI, Knowledge, Observability, World Simulation, Plugins) would make the hottest path in the product the most expensive one. This removes a concept rather than adding one, and closes the backpressure open question.