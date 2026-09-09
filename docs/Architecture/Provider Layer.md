# Provider Layer

> Status: Designed · Owner ADR: [[../ADR/0007-provider-abstraction]] · Depends: [[../ADR/0005-capability-first-architecture]], [[../ADR/0006-conversation-domain-model]]

## Purpose
Turn any LLM backend into an interchangeable AIOS Provider. Engine sends a Conversation, gets Conversation-shaped results back, and never learns whose model it was.

## Responsibilities
- Translate outgoing AIOS Conversation to native request, and native response/stream back to canonical domain types (ContentBlocks, ToolCalls, Thinking, StreamingEvents, Citations).
- Publish a Provider Profile plus per-Model Profiles (capabilities, limits, context window, modalities, cost).
- Own transport: auth, endpoints, retries, rate-limit handling, timeouts.
- Surface health/availability (is Ollama running? is the key valid?).
- Isolate provider-only features in Adapter Extensions.

## NOT responsible for
- Choosing which provider/model runs (Orchestration, via capability query).
- Prompt content, agent identity, system prompts (Agent Runtime).
- Memory, context assembly, token budgeting (Context Engine).
- Tool execution (emits ToolCall requests; the Tool layer runs them).
- Conversation storage/history (Persistence).
- Cost accounting/limits (reports usage; Cost subsystem decides).
- Discovery/install of local backends (Integration layer detects Ollama/LM Studio, hands a config to Provider).

Sharp line: **Provider = pure translator + transport. Nothing else.**

## Communication
- Inbound: `Provider` trait, called by the engine. Command-style: canonical Conversation + RequestOptions.
- Outbound: canonical Response, or a stream of StreamingEvents on the event bus (deltas, thinking, tool-call-start, done).
- Profiles queried by the Capability System (read-only, cacheable).
- Usage/health emitted as events (Cost + UI consume).
- One-way dependency: Provider depends only on the Domain Kernel. Never calls Tools, Memory or UI.

## Interface sketch
```
Provider (trait):
  fn profile() -> ProviderProfile
  fn models() -> Vec<ModelProfile>
  fn health() -> HealthStatus
  fn send(Conversation, RequestOptions) -> Result<Response>
  fn stream(Conversation, RequestOptions) -> Stream<StreamingEvent>
  fn extensions() -> Option<AdapterExtensions>   // walled escape hatch

ProviderRegistry: live providers, indexed BY CAPABILITY not name
```
Each backend = one adapter implementing `Provider`; adapters are the only place native SDK types exist. Multi-provider routing/fallback = a composite Provider wrapping others.

## Future scalability
- New backend = new adapter file, zero engine change (endpoint/auth data-driven; code only for protocol translation).
- New modality = extend ContentBlock + Profile once; adapters opt in via capability; old adapters keep working.
- Routing/fallback/load-balancing = composite Provider, free.
- Headless/CLI reuse: trait has no UI assumption.

## Risks
- Domain model too narrow -> escape hatch becomes default -> abstraction rot. *Mitigation:* deliberate superset; review each Extension use.
- Streaming shape divergence across providers. *Mitigation:* canonical StreamingEvent enum designed before the first adapter.
- Capability/Profile drift. *Mitigation:* versioned Profiles; runtime probing for local backends.
- Tool-call semantic differences (parallel calls, IDs). *Mitigation:* canonical ToolCall/ToolResult owns the contract.

## Assumptions
- Domain Kernel superset + capability vocabulary are rich enough for real adapters.

## Open questions
- Retry/rate-limit policy: per-adapter vs. shared engine middleware.
- Cost data origin when providers do not report token usage.

---

## Streaming channel (clarified 2026-07-25, ADR-0015)
`Provider::stream` returns canonical `StreamingEvent`s (ADR-0006) directly to the calling Character Instance, which forwards them to the UI. **Token-level events never enter the [[Activity Stream]]** - that stream carries only coarse turn-level facts (`ProviderRequestStarted` / `ProviderRequestCompleted`).