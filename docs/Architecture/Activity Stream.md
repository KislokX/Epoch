# Activity Stream

> Status: Designed · Owner ADR: [[../ADR/0015-activity-stream]] · Horizon: in-memory NOW; Activity Recorder interface DESIGN NOW
> Principle: **Commands cause; Activities record** ([[Principles & Runway]]).

The nervous system of AIOS. In-memory pub/sub of immutable Activities. Communication, **not** persistence.

## Two interaction kinds
- **Commands** - synchronous, caller needs a response. The turn loop runs on Commands.
- **Activities** - asynchronous, fire-and-forget, "something happened." Emitted alongside Commands.

```
Commands -> Execution
     |
     v
Activities (in-memory)
     |-> UI
     |-> Knowledge Engine -> KnowledgeObjects -> Persistence
     |-> Automation Engine
     |-> Observability (Activity Recorder = future)
     |-> Plugins
```
Persistence never subscribes. Repositories remain the source of truth.

## Activity (Kernel type)
id · type · timestamp · scope ids (Session/Instance/Project) · correlation/causation id · **Visibility** (Public/Internal) · **Importance** (Debug/Normal/Critical) · immutable payload.

Example types: AgentStarted, ContextComposed, ProviderRequestStarted/Completed, ToolExecutionStarted/Completed, KnowledgeCreated, ConversationUpdated, SnapshotCreated, TrustDecisionMade.

## Hard rules
- **No self-derivation.** A consumer never derives from Activities it emitted itself. The Knowledge Engine emits `KnowledgeCreated` as **Visibility: Internal** and excludes its own emissions - knowledge derivation cannot recurse.
- No subsystem may depend on an Activity existing after emission.
- Activities are transient; Knowledge is persistent (Knowledge Engine derives it).
- Transport is swappable (memory now; Kafka/SQLite/network later) precisely because nothing depends on durability.

## Consumers
UI · Knowledge Engine · Automation Engine · [[Observability]] · Plugins · Debug Tools.

## Token streaming does not belong here
Per-token streaming uses the **`StreamingEvent` channel** already defined in the [[Domain Kernel]] (ADR-0006) and returned by `Provider::stream` (ADR-0007): Provider -> Instance -> UI. The Activity Stream carries only **coarse turn-level facts** (`ProviderRequestStarted` / `ProviderRequestCompleted`). Streaming is the response to a Command, not an observation.

## NOT responsible for
- Persistence (ADR-0014).
- Being a source of truth or an event-sourcing store.
- Guaranteed delivery in Phase 1.

## Horizon
- NOW: in-memory pub/sub, emission, subscription, metadata filtering.
- DESIGN NOW: Activity Recorder interface (durable append-only log).
- VISION: replay / event-sourcing.

## Open questions
- Delivery semantics (sync-dispatch vs. queued).
- Delivery ordering guarantees across subscribers.
