# ADR-0011: Definition-Driven Runtime (Runtime over Configuration)

> **AMENDED by [[0015-activity-stream]]** - read the Amendment at the end of this file before implementing. Text above the Amendment may be superseded.

- Status: Accepted
- Date: 2026-07-23
- Depends on: [[0003-engine-presentation-separation]], [[0005-capability-first-architecture]], [[0008-execution-engine]]
- Related architecture: [[Definition Runtime]], [[Definition Registry]], [[Domain Kernel]]
- Horizon: Character schema + Registry + stateless resolve + Instance core **IMPLEMENT NOW**; memory/knowledge/trust binding, persist/restore, generic Definition trait **DESIGN NOW**; automation policies + non-Character types **VISION**

## Context
Agents must not be hardcoded classes. The engine is data-driven and hot-reloadable (ADR-0003). We need behavior to emerge from structured, editable data — and to generalize beyond "agents" so future concepts (Teams, Workflows, Plugins, Assistants) reuse the same machinery.

## Decision
The runtime **executes Definitions**, not hardcoded characters. New principle: **Runtime over Configuration** — behavior emerges from structured Definitions interpreted by the Runtime.

Phase-1 Definition type is **Character**. Three distinct concepts:

- **Character Definition** — immutable data: Identity, Role, Prompt, Requested Capabilities, Preferred Models, Knowledge Scope, Automation Policies, Trust Policies, UI Metadata.
- **Character Runtime** — **stateless** resolver: Definition + ResolveContext -> ResolvedBinding -> Character Instance.
- **Character Instance** — the **only** stateful component; a live execution owning: Active Conversation, Memory Handle, Streaming State, Cancellation Token, Execution Queue, Pending Tool Calls, Temporary Context, Event Emission.

Definitions are reusable across projects. Multiple Instances may be created from one Definition with **no shared runtime state**. An Instance can be persisted and restored **without modifying its Definition**.

Two subsystems by change-rate: **Definition Registry** (load/validate/watch/version, hot-reload) and **Character Runtime** (resolve/instantiate). Registry knows files; Runtime knows execution.

Reconciliation with Capability-First (ADR-0005): Requested Capabilities are the hard requirement; Preferred Models only rank within the capability-matched set; user preference + availability break ties.

## Why this is best
- Swappable "party members" without recompiling — matches the JRPG north star.
- Immutable Definition + stateful Instance makes hot-reload safe: an Instance snapshots its Definition at spawn, so edits affect new Instances only; running Instances never mutate mid-turn.
- One Runtime for all future Definition types — extensibility with no Runtime change.

## Alternatives considered
- **Hardcoded agent classes** — not swappable, violates the data-driven mandate. Rejected.
- **Stateless-only runtime** — awkward for streaming, long-running, Autonomous mode; pushes state onto callers. Rejected.
- **Stateful-only runtime** — couples resolution to live state, kills Definition reuse across projects. Rejected.

## Consequences
- Character schema must be defined + versioned in the Domain Kernel now (even fields whose subsystems ship later).
- The Instance is a well-bounded stateful actor with an explicit lifecycle (spawn/suspend/resume/dispose, snapshot/restore).
- Definitions (authored INPUT) are distinct from Knowledge Objects (emitted OUTPUT); a running Character emits AgentActivity, the Character is not itself knowledge.

## Assumptions
- A stable Character schema can express Phase-1 needs and absorb later fields as declared-but-inert.
- Resolution can be made reproducible by recording the chosen binding.

## Risks
- Schema churn breaks saved definitions. *Mitigation:* version + migrate; validate on load.
- Instance state sprawl/leaks. *Mitigation:* single-owner lifecycle; explicit persistence boundary.
- Resolution nondeterminism as availability shifts. *Mitigation:* record chosen binding into the Instance + a KnowledgeObject.

## Open questions
- Definition file format (TOML/YAML/JSON) and vault location.
- Instance-state persistence shape (with Persistence subsystem).
- Generic `Definition` trait shape when the second Definition type appears (DESIGN NOW).

---

## Amendment (ADR-0015, applied 2026-07-25)
The Runtime does **not** emit KnowledgeObjects. The chosen ResolvedBinding is recorded **in the Instance** (unchanged) and published on the **`AgentStarted` Activity** payload. The Knowledge Engine derives any persistent record from the stream. Corrects "record chosen binding into the Instance + a KnowledgeObject".
## Clarification (applied 2026-07-25): one store for Definitions
The Definition Registry is the **repository implementation for Definitions over the vault-file Storage Adapter** (ADR-0014), not a second store. One source of truth: the file in the vault, reached through the Persistence Contract.