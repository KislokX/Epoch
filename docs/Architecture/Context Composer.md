# Context Composer

> Status: Designed · Owner ADR: [[../ADR/0012-context-composer]] · Horizon: mixed (see ADR)
> Principle: **Context is composed, never concatenated** ([[Principles & Runway]]).

## Purpose
Compose a canonical Conversation from structured Context Blocks under the target model's token budget - explainably. The turn-loop step a [[Definition Runtime]] Instance calls before every Provider call.

## Shape
**Stateless.** `compose(instance_state, providers, model_profile) -> (Conversation, ContextReport)`. Same inputs -> same output.

## Context Providers (plug-in pipeline)
Each provider yields canonical **Context Blocks**. Phase-1 set (NOW): Character Definition, Conversation, Available Capabilities, Goals, Files. Deferred (DESIGN NOW): Memory, Knowledge Objects, Workspace State, Execution Context, Trust Context. New providers register without changing the Composer.

## Context Block metadata
Priority · Required · Compressible · Summarizable · Cacheable · Lifetime.

## Selection Strategy (pluggable)
Phase 1: **static priority tiers**. Future: semantic / temporal / project / hybrid ranking - swap the Strategy, not the Composer.

## Reduction pipeline (deterministic)
1. Compress eligible blocks. *(NOW)*
2. Summarize eligible blocks. *(DESIGN NOW - a model call; cost/latency/recursion)*
3. Drop lowest-priority optional blocks.
**Required blocks are never removed.**

## Model-aware budget
Runs after capability resolution. Reads the resolved Model Profile ([[Capability System]]) for context window + tokenizer. Budgeting is impossible without the chosen model.

## Context Report (every turn)
Carried on the `ContextComposed` Activity ([[Activity Stream]]): included / summarized / compressed / excluded + why. The [[Knowledge Engine]] subscribes and decides whether it becomes a persistent KnowledgeObject. Understandable Autonomy - the AI explains its context decisions.

## NOT responsible for
- Provider-native translation ([[Provider Layer]]).
- Model selection (Capability System, upstream).
- Owning memory/knowledge/file data (Providers supply it).
- Storing history (Persistence).

## Turn loop (where this sits)
resolve model (capability) -> **Context Composer** -> Provider translate+call -> stream -> ToolCalls -> [[Execution Engine]] (Trust-gated) -> emit AgentActivity + Context Report.

## Risks
- Summarization cost/recursion -> DESIGN NOW; Phase 1 compress+drop.
- Budget estimation inaccuracy -> Model Profile tokenizer + margin.
- Nondeterminism -> stateless + Context Report.

## Open questions
- Phase-1 compression technique.
- Context Report: own KnowledgeType vs. part of AgentActivity.
- Cacheable-block semantics.
