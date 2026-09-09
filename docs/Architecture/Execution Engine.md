# Execution Engine

> Status: Designing · Owner ADR: [[../ADR/0008-execution-engine]] · Horizon: IMPLEMENT NOW

## Purpose
Provide one caller-agnostic way to run any **Executable Capability** in AIOS, and own every cross-cutting execution concern in one place.

## Core concept — Executable Capability
A capability declares only: **Profile · Capabilities · Input Schema · Output Schema**, and executes. It **never knows its caller** (agent, automation, orchestration, plugin, scheduler, CLI, other subsystem). Tools are the first kind; Providers/Integrations become invokable capabilities over time.

## Responsibilities (the Engine, not the capability)
- Permission gating (delegates to [[Trust Engine]]).
- Execution context, cancellation, retries, progress.
- Logging, events (on the bus), auditing, telemetry.
- Emit a Knowledge Object per execution ([[Knowledge Engine]]).

## NOT responsible for
- The capability's actual work (self-contained in the capability).
- Deciding *which* capability to run (Orchestration / Agent Runtime, via capability query).
- Trust policy itself (Trust Engine owns it; Execution Engine enforces the verdict).

## Communication
- Callers invoke `execute(capability, input, context)`.
- Depends on Domain Kernel (schemas, capability descriptors) + Trust Engine (permission) + Knowledge Engine (emit).
- Emits progress/result/error as events; long-running capabilities stream progress events.

## Future scalability
- New caller = free (contract is caller-agnostic).
- New capability kind (Provider/Integration as capability) = implements the same contract.

## Earn Complexity
- IMPLEMENT NOW: execute + permission gate + result + Knowledge Object emit.
- DESIGN NOW: rich telemetry/audit surface, cancellation/progress streaming.
- VISION: distributed/remote execution.

## Risks
- God-object drift. *Mitigation:* owns concerns, never business logic.
- Schema mismatch between LLM + programmatic callers. *Mitigation:* structured, self-describing Input/Output schema.

## Open questions
- Schema format (JSON Schema vs. typed Rust + generated).
- Cancellation/progress model.

---

## Amendment (ADR-0015)
The Execution Engine emits **Activities** (ToolExecutionStarted/Completed) to the [[Activity Stream]] instead of emitting KnowledgeObjects directly. The synchronous result still returns to the caller (Command); Activities inform observers asynchronously.
