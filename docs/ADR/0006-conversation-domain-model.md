# ADR-0006: AIOS Conversation Domain Model

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0003-engine-presentation-separation]]
- Related architecture: [[Domain Kernel]], [[Provider Layer]]

## Context
Each provider speaks its own protocol (Anthropic messages, OpenAI responses, Gemini payloads, Ollama). If provider shapes leak into the engine, agents and orchestration become coupled to vendors — the exact rewrite risk we are avoiding.

## Decision
The engine defines its own **AIOS Conversation Domain Model** and understands nothing else. Providers are **translators** between their native protocol and this model, both directions. The canonical model represents:

- Conversations, Messages, Content Blocks
- Tool Calls, Tool Results
- Files, Images, Audio
- Streaming Events, Thinking
- Metadata, Citations, Artifacts

Raw provider payloads are **never** exposed to the engine. Provider-specific features live in **Adapter Extensions**, walled off from the core.

## Why this is best
- The engine never knows whether a conversation came from Claude, GPT, Gemini or a local model — one of the strongest guarantees in the system.
- Adding/replacing a provider is isolated to its adapter.
- A rich canonical model (thinking, citations, artifacts, multimodal) future-proofs against features other providers add later.

## Alternatives considered
- **Normalize Message/Response only** — leaks as soon as you hit tool-calls, thinking, artifacts. Rejected.
- **Pass-through native formats** — destroys the abstraction; vendor detail spreads everywhere. Rejected.
- **Escape hatch as default** — erodes the abstraction over time. Rejected in favor of walled Adapter Extensions used only when absolutely necessary.

## Consequences
- The domain model must be designed as a deliberate **superset** and versioned in the Domain Kernel.
- Each provider adapter carries real translation work (esp. streaming + tool-call semantics).
- Advanced provider-specific features require an explicit, reviewed Extension — friction by design.

## Assumptions
- A canonical superset can express current and near-future provider concepts.
- Streaming can be normalized into a single StreamingEvent vocabulary.

## Risks
- Model too narrow → Extensions overused → abstraction rot. *Mitigation:* model the superset deliberately; treat each Extension use as a design smell to review.
- Lossy translation (native concept with no canonical home). *Mitigation:* Metadata carries non-semantic extras; genuine gaps trigger a model revision, not a leak.

## Open questions
- Exact StreamingEvent enum and ToolCall/ToolResult schema — finalize before the first adapter.
- Versioning/migration policy for the domain model.
