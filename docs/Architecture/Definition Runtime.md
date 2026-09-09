# Definition Runtime

> Status: Designed · Owner ADR: [[../ADR/0011-definition-runtime]] · Horizon: mixed (see ADR)
> Principle: **Runtime over Configuration** ([[Principles & Runway]]).

## Purpose
Execute behavior from data. Interpret Definitions into live executions. Phase-1 Definition type: **Character**. The same Runtime serves future types (Teams, Workflows, Plugins, Assistants) unchanged.

## The triad
- **Character Definition** — immutable authored data. Fields: Identity, Role, Prompt, Requested Capabilities, Preferred Models, Knowledge Scope, Automation Policies, Trust Policies, UI Metadata.
- **Character Runtime** — **stateless** resolver. `resolve(Definition, ResolveContext) -> ResolvedBinding`; `instantiate(ResolvedBinding) -> CharacterInstance`. Same inputs -> same resolution.
- **Character Instance** — **only** stateful component. Owns: Active Conversation, Memory Handle, Streaming State, Cancellation Token, Execution Queue, Pending Tool Calls, Temporary Context, Event Emission. Lifecycle: spawn/suspend/resume/dispose + snapshot/restore.

## Resolution (capability-first)
Requested Capabilities = hard filter (via [[Capability System]]) -> Preferred Models rank within the matched set -> user preference + availability break ties. The chosen binding is recorded for reproducibility.

## What an Instance does per turn
Assemble context ([[Context Engine]] target) -> select + call Provider ([[Provider Layer]]) -> receive ToolCalls -> dispatch via [[Execution Engine]] (Trust-gated, [[Trust Engine]]) -> emit AgentActivity ([[Knowledge Engine]]) + stream events on the bus.

## NOT responsible for
- Provider protocol, tool execution, trust policy, context assembly, definition storage, instance-byte storage — all delegated. Runtime holds no state; Instance never mutates its Definition.

## Hot-reload safety
An Instance snapshots its (immutable) Definition at spawn. Editing a Character file affects **new** Instances only; running party members are untouched.

## Horizon
- NOW: schema (all fields), stateless resolve (Identity/Role/Prompt/Capabilities/PreferredModels/UI), Instance core (conversation/streaming/cancellation/events/exec-queue), emit AgentActivity.
- DESIGN NOW: Memory-handle binding, Knowledge Scope, per-character Trust binding, Instance persist/restore, generic Definition trait.
- VISION: Automation Policies, non-Character Definition types.

## Risks
- Schema churn -> version + migrate, validate on load.
- Instance state sprawl -> single-owner lifecycle + persistence boundary.
- Resolution nondeterminism -> record chosen binding into the Instance; publish it on the `AgentStarted` Activity.

## Open questions
- Definition file format + vault location.
- Instance-state persistence shape.
- Generic Definition trait shape (second type).

---

## RESOLVED 2026-07-25 by [[../ADR/0018-world-simulation]]
Presence data (Routine, Idle behavior, Home place, Relationships, Emotional range) now lives in the **World Simulation** as `PresenceProfile` / `PresenceState` - NOT in `CharacterDefinition`. Identity and presence stay separate contracts. Original note below for history.

### Original note (Character Bible)
[CHARACTER_BIBLE.md](../../CHARACTER_BIBLE.md) is the canonical identity source; CharacterDefinition is its engine projection. Bible fields **Routine, Idle behavior, Favorite places, Relationships, Emotional range** have no home in the current schema - they are world-simulation data. A **Presence** block will likely be added to CharacterDefinition when the World simulation is designed. Deliberately not invented yet (Earn Complexity).

Related guardrail: Characters must be **missable**, and absence must be **true** - a Character is elsewhere only when a real Instance is running real work. The World never stages fake busyness.

