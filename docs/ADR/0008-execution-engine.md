# ADR-0008: Executable Capability & Execution Engine

> **AMENDED by [[0015-activity-stream]]** - read the Amendment at the end of this file before implementing. Text above the Amendment may be superseded.

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0004-provider-tool-integration-trichotomy]], [[0005-capability-first-architecture]]
- Related architecture: [[Execution Engine]], [[Domain Kernel]]
- Horizon: **IMPLEMENT NOW** (core contract + engine); telemetry/audit surface **DESIGN NOW**

## Context
Tools are invoked by many callers: agents (LLM emits a ToolCall), automations, the orchestration engine, plugins, a future scheduler, CLI commands, other subsystems. If each caller has its own tool surface, we get duplicated contracts, split permissions, and a fragmented capability model.

## Decision
The engine thinks in **Executable Capabilities**, not "agent tools" vs. "engine tools". A capability **never knows who invoked it** — the execution contract is identical for every caller.

A capability declares only: **Profile, Capabilities, Input Schema, Output Schema**, and executes.

A single **Execution Engine** owns every cross-cutting execution concern:
- permissions (via the Trust Engine, ADR-0009)
- execution context, cancellation, retries, progress
- logging, events, auditing, telemetry
- emitting a Knowledge Object per execution (ADR-0010)

Tools are the first kind of Executable Capability; Providers and Integrations become invokable through the same model over time.

## Why this is best
- One execution model for the whole OS — one place to secure, observe, cancel, audit.
- Caller-agnostic capabilities compose freely (agent today, scheduler tomorrow) with zero capability changes.
- Directly enables the Trust Engine and Knowledge Engine, which hook the single execution path.

## Alternatives considered
- **Separate agent-tools vs. engine-actions** — duplication, two permission systems, split capability model. Rejected.
- **Tools own their own permission/retry/logging** — cross-cutting logic scattered, inconsistent, unsafe. Rejected.

## Amendment 2026-08-02 — outside tools, and why they are all treated as the ceiling

The MCP client is built. Every tool an outside server offers becomes an **ordinary capability**:
same `Descriptor`, same `explain`, same `decide()`, same record. Nothing downstream can tell one
from a capability Epoch wrote, which is the claim this ADR made when it chose a contract over a
list — and it held with no new abstraction.

One finding, and it is the load-bearing one.

**MCP declares no effects.** This ADR derives risk *from* effects precisely so that nobody can
label a delete as low-risk. MCP does offer hints — `readOnlyHint`, `destructiveHint`,
`idempotentHint` — and **they are refused**. A hint is a third party grading its own risk, which
is the exact claim this ADR says must not be expressible, arriving from the least trustworthy
place it could: a program the user installed from somewhere else.

So an outside tool declares **every effect and a permanent reversal**. It reads, writes, deletes,
executes and reaches the network until proven otherwise, and it can prove nothing.

The consequence is deliberate and worth stating rather than discovering: **every MCP tool asks
the first time**, in every mode but Auto. That is not a degraded experience — it is the correct
one. The Trust store already remembers a standing answer (ADR-0009), so it asks once rather than
every time, and the user grants a *named* outside program a *named* permission. Believing
`readOnlyHint: true` would instead have let a tool that deletes through a mode the user had set
to ask first.

**A tool whose parameters cannot be modelled is left out with a reason**, not offered broken.
`ValueKind` has no array or object, so a tool that *requires* one would fail on first use — the
same argument that gives a World with no Project Root an empty registry rather than capabilities
that fail when reached for. An *optional* parameter of an unknown shape costs only itself.

**An outside tool's result is `told`, never `made`.** Whatever it changed, Epoch did not see it
happen and holds no artifact — and evidence is what a run left behind, not what it reported
(ADR-0025). Recording a stranger's sentence as evidence would put it into a Quest's History as
proof.

## Consequences
- Input/Output schemas must serve both LLM callers and programmatic callers (structured, self-describing).
- The Execution Engine becomes a central, well-tested chokepoint (good for safety, must not become a god object — it orchestrates concerns, capabilities do the work).

## Assumptions
- A schema-driven capability contract can express Tools now and Providers/Integrations later.

## Risks
- Execution Engine scope creep. *Mitigation:* it owns *concerns* (permission, retry, telemetry), never business logic; capabilities stay self-contained.

## Open questions
- Schema format for Input/Output (JSON Schema vs. typed Rust + generated schema).
- Cancellation/progress model for long-running capabilities (streaming events).

---

## Amendment (ADR-0015, 2026-07-23)
Executions no longer emit KnowledgeObjects directly. They emit immutable **Activities** to the [[Activity Stream]] (e.g. ToolExecutionStarted/Completed). The Knowledge Engine subscribes and derives KnowledgeObjects. Repositories remain the source of truth; nothing may depend on an Activity after emission.

---

## Amendment (2026-07-31, from building it) — progress, and the open question it closes

> Closes the open question *"Cancellation/progress model for long-running capabilities
> (streaming events)"*. The answer was smaller than the question implied.

### One extra method, defaulted to silence

```rust
fn run_watched(&self, arguments: &Arguments, saying: &mut dyn FnMut(&str))
    -> Result<Outcome, CapabilityError> { self.run(arguments) }
```

Most capabilities are over before there is anything to report — a file is read or it is not — so
implementing it is opt-in and the other seven were untouched. It exists for the ones where **the
waiting is the experience**: a build, a test run, a large clone. A test asserts that a
non-streaming capability still narrates nothing, because the cost of this addition to everything
else has to stay zero and stay proven.

### Lines are observations; `Outcome` is still the result

Streaming does not replace returning. What a run *left behind* is `Outcome::evidence` and nothing
else (ADR-0025). A surface that treated a program's chatter as its result would be reporting
noise as fact, so `Step::Said` is deliberately a different variant from `Step::Used` and the
Terminal renders them differently.

They travel on the **same channel** as `Using` and `Used`. A surface following work wants them
interleaved in the order they happened; two channels would make ordering the surface's problem.

### The real prize was cancellation, which had never worked

`run_command` used `Command::output()`, which waits. The 180-second limit was therefore
*measured and not enforced*: a program waiting on input nobody would ever type held the turn
open forever, and the code could only say afterwards that it had taken too long.

Spawning made the deadline real. Past it the child is killed, and **what it printed before dying
is still returned** — a program that hung halfway should be able to say which half it finished.

Two reader threads rather than polling both pipes: reading one while the other fills its buffer
is how a process deadlocks against its own output, and that is a defect that only appears once
somebody runs something talkative.

### What this does not do

No cancellation *by the user* — there is no stop button, only a deadline. That needs a handle on
the running child reachable from a surface, and it should be built when there is a surface asking
for it rather than in anticipation.
